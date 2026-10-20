use super::{design, index, quality, state};
use crate::{
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};
use tokio_util::sync::CancellationToken;
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Step {
    pub id: String,
    pub tool: String,
    #[serde(default)]
    pub args: Value,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct StartArgs {
    pub name: String,
    #[serde(default)]
    pub folder: String,
    pub steps: Vec<Step>,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ControlArgs {
    pub id: String,
    pub action: String,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct StatusArgs {
    pub id: String,
    pub include_results: Option<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Effect {
    step: String,
    folder: String,
    path: String,
    sha256: String,
    before: Option<String>,
    previous_version: Option<String>,
    created: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub roots: Vec<PathBuf>,
    pub steps: Vec<Step>,
    pub status: String,
    pub completed: usize,
    pub results: Value,
    pub error: Option<String>,
    pub updated: u64,
    pub undo_completed: usize,
    #[serde(default)]
    effects: Vec<Effect>,
}
struct Run {
    job: Job,
    cancel: CancellationToken,
    reason: Arc<AtomicU8>,
}
static RUNS: OnceLock<Mutex<HashMap<String, Run>>> = OnceLock::new();
fn registry() -> &'static Mutex<HashMap<String, Run>> {
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}
tokio::task_local! {static CANCEL:CancellationToken;}
tokio::task_local! {static STEP:(String,String);}
pub fn check_write() -> Result<(), String> {
    if CANCEL.try_with(|c| c.is_cancelled()).unwrap_or(false) {
        Err("Workflow suspendu : écriture annulée".into())
    } else {
        Ok(())
    }
}
fn persist(job: &Job) -> Result<(), String> {
    state::reserve(&state::root()?.join("jobs"), 6_000_000, 128 * 1024 * 1024)?;
    state::write_json(
        &state::root()?.join("jobs").join(format!("{}.json", job.id)),
        job,
    )
}
fn read(id: &str) -> Result<Job, String> {
    state::valid_id(id)?;
    let path = state::root()?.join("jobs").join(format!("{id}.json"));
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 6_000_000 {
        return Err("Workflow trop volumineux".into());
    }
    let mut job: Job = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if job.status == "running" {
        job.status = "interrupted".into()
    }
    Ok(job)
}
fn authorized(folders: &[SharedFolder], job: &Job) -> Result<(), String> {
    let roots: Vec<_> = folders
        .iter()
        .filter_map(|f| f.path.canonicalize().ok())
        .collect();
    if job.roots.iter().any(|r| !roots.contains(r)) {
        return Err("Un dossier du workflow n’est plus partagé".into());
    }
    Ok(())
}
fn summary(job: &Job) -> Value {
    json!({"id":job.id,"name":job.name,"status":job.status,"completed":job.completed,"steps":job.steps.len(),"progress":job.completed*100/job.steps.len().max(1),"error":job.error,"updated":job.updated,"undo_completed":job.undo_completed})
}
pub fn status(folders: &[SharedFolder], args: StatusArgs) -> Result<Value, String> {
    let registry = registry().lock().map_err(|_| "Workflows occupés")?;
    let job = if let Some(run) = registry.get(&args.id) {
        run.job.clone()
    } else {
        read(&args.id)?
    };
    authorized(folders, &job)?;
    let mut value = summary(&job);
    if args.include_results.unwrap_or(false) {
        let results = serde_json::to_string(&job.results).map_err(|e| e.to_string())?;
        if results.len() > 100_000 {
            value["results_truncated"] = json!(true);
            value["results_excerpt"] = json!(results.chars().take(100_000).collect::<String>())
        } else {
            value["results"] = job.results
        }
    }
    Ok(value)
}
pub fn list() -> Result<Value, String> {
    let dir = state::root()?.join("jobs");
    let mut jobs = vec![];
    if dir.exists() {
        for file in fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            if file.path().extension().is_some_and(|e| e == "json") {
                let id = file
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if let Ok(job) = read(&id) {
                    jobs.push(job)
                }
            }
        }
    }
    let runs = registry().lock().map_err(|_| "Workflows occupés")?;
    for job in &mut jobs {
        if let Some(r) = runs.get(&job.id) {
            *job = r.job.clone()
        }
    }
    jobs.sort_by_key(|j| std::cmp::Reverse(j.updated));
    jobs.truncate(100);
    Ok(json!({"jobs":jobs.iter().map(summary).collect::<Vec<_>>()}))
}
fn references(value: &Value, results: &Value) -> Result<Value, String> {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("$ref") => {
            let key = map["$ref"]
                .as_str()
                .ok_or("Référence attendue sous forme de texte")?;
            let mut parts = key.split('.');
            let mut result = results
                .get(parts.next().unwrap_or(""))
                .ok_or("Étape de référence non terminée")?;
            for part in parts {
                result = if let Ok(i) = part.parse::<usize>() {
                    result.get(i)
                } else {
                    result.get(part)
                }
                .ok_or("Champ de résultat absent")?
            }
            Ok(result.clone())
        }
        Value::Object(map) => Ok(Value::Object(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), references(v, results)?)))
                .collect::<Result<_, String>>()?,
        )),
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|v| references(v, results))
                .collect::<Result<_, _>>()?,
        )),
        _ => Ok(value.clone()),
    }
}
async fn execute(
    folders: Vec<SharedFolder>,
    step: &Step,
    mut args: Value,
    default: &str,
) -> Result<Value, String> {
    check_write()?;
    if !args.is_object() {
        return Err("args doit être un objet JSON".into());
    }
    if args.get("folder").is_none() {
        args["folder"] = json!(default)
    }
    match step.tool.as_str(){
 "create_document"=>design::create(&folders,serde_json::from_value(args).map_err(|e|e.to_string())?).await,
 "write_text_file"=>writing::text(&folders,args["folder"].as_str().unwrap_or(""),args["path"].as_str().ok_or("path requis")?,args["content"].as_str().ok_or("content requis")?,args["expected_sha256"].as_str()),
 "write_spreadsheet"=>{let cells:Vec<crate::generate::CellEdit>=serde_json::from_value(args["cells"].clone()).map_err(|e|e.to_string())?;crate::generate::spreadsheet(&folders,args["folder"].as_str().unwrap_or(""),args["output_path"].as_str().ok_or("output_path requis")?,args["input_path"].as_str(),args["expected_sha256"].as_str(),&cells)},
 "read_file"=>library::extract(&folders,args["folder"].as_str().unwrap_or(""),args["path"].as_str().ok_or("path requis")?,args["page"].as_u64().unwrap_or(1)as usize).map(|mut v|{if let Some(text)=v["text"].as_str(){v["text"]=json!(text.chars().take(crate::limits::DEFAULT_TEXT_CHARS).collect::<String>())}v}),
 "index_folder"=>index::build(&folders,serde_json::from_value(args).map_err(|e|e.to_string())?),
 "search_index"=>index::query(&folders,serde_json::from_value(args).map_err(|e|e.to_string())?),
 "ocr_file"=>crate::ocr::recognize(folders,serde_json::from_value(args).map_err(|e|e.to_string())?).await,
 "extract_document_layout"=>super::ocr_layout::extract(folders,serde_json::from_value(args).map_err(|e|e.to_string())?).await,
 "review_document"=>quality::inspect(&folders,args["folder"].as_str().unwrap_or(""),args["path"].as_str().ok_or("path requis")?),
 "create_archive"=>crate::archives::execute(&folders,crate::archives::Command::Create{folder:args["folder"].as_str().unwrap_or("").into(),paths:serde_json::from_value(args["paths"].clone()).map_err(|e|e.to_string())?,output_path:args["output_path"].as_str().ok_or("output_path requis")?.into()}),
 _=>Err("Étape autorisée : create_document, write_text_file, write_spreadsheet, read_file, index_folder, search_index, ocr_file, extract_document_layout, review_document, create_archive".into())
 }
}
fn launch(
    folders: Vec<SharedFolder>,
    mut job: Job,
    parent: CancellationToken,
) -> Result<Value, String> {
    let mut runs = registry().lock().map_err(|_| "Workflows occupés")?;
    if runs.values().filter(|r| r.job.status == "running").count() >= 2 {
        return Err("Deux workflows simultanés maximum".into());
    }
    if runs.get(&job.id).is_some_and(|r| r.job.status == "running") {
        return Err("Workflow déjà actif".into());
    }
    job.status = "running".into();
    job.error = None;
    job.updated = state::now();
    persist(&job)?;
    if parent.is_cancelled() {
        return Err("Le serveur s’arrête : workflow refusé".into());
    }
    let cancel = parent.child_token();
    let reason = Arc::new(AtomicU8::new(1));
    let response = summary(&job);
    runs.insert(
        job.id.clone(),
        Run {
            job: job.clone(),
            cancel: cancel.clone(),
            reason: reason.clone(),
        },
    );
    drop(runs);
    tokio::spawn(CANCEL.scope(cancel.clone(), run_job(folders, job, cancel, reason)));
    Ok(response)
}
pub fn journal_intent(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    sha: &str,
    before: Option<&str>,
    previous: Option<&str>,
) -> Result<(), String> {
    let Some((id, step)) = STEP.try_with(Clone::clone).ok() else {
        return Ok(());
    };
    let mut runs = registry()
        .lock()
        .map_err(|_| "Journal de workflow occupé")?;
    let run = runs.get_mut(&id).ok_or("Workflow absent du journal")?;
    let folder = library::select_folder(folders, folder)?.id.clone();
    if run
        .job
        .effects
        .iter()
        .any(|e| e.step == step && e.folder == folder && e.path == path && e.sha256 == sha)
    {
        return Ok(());
    }
    run.job.effects.push(Effect {
        step,
        folder,
        path: path.into(),
        sha256: sha.into(),
        before: before.map(str::to_owned),
        previous_version: previous.map(str::to_owned),
        created: before.is_none(),
    });
    persist(&run.job)
}
fn effect_state(folders: &[SharedFolder], e: &Effect) -> Result<bool, String> {
    let path = writing::destination(folders, &e.folder, &e.path)?;
    if !path.exists() {
        return if e.created {
            Ok(false)
        } else {
            Err("Un fichier à restaurer a disparu".into())
        };
    }
    let hash = writing::hash(&fs::read(path).map_err(|e| e.to_string())?);
    if hash == e.sha256 {
        return Ok(true);
    }
    if e.before.as_deref() == Some(&hash) {
        return Ok(false);
    }
    Err(format!("{} a changé depuis le workflow", e.path))
}
async fn run_job(
    folders: Vec<SharedFolder>,
    mut job: Job,
    cancel: CancellationToken,
    reason: Arc<AtomicU8>,
) {
    while job.completed < job.steps.len() {
        if cancel.is_cancelled() {
            job.status = if reason.load(Ordering::SeqCst) == 2 {
                "cancelled"
            } else {
                "paused"
            }
            .into();
            break;
        }
        let step = job.steps[job.completed].clone();
        let value = match references(&step.args, &job.results) {
            Ok(v) => v,
            Err(e) => {
                job.status = "failed".into();
                job.error = Some(e);
                break;
            }
        };
        let effect = job
            .effects
            .iter()
            .rev()
            .find(|e| e.step == step.id)
            .cloned();
        let recovered = if let Some(e) = effect {
            match effect_state(&folders, &e) {
                Ok(true) => Some(Ok(
                    json!({"path":e.path,"sha256":e.sha256,"created":e.created,"previous_version":e.previous_version,"job_folder":e.folder,"recovered_after_interrupt":true}),
                )),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        } else {
            None
        };
        let outcome = if let Some(result) = recovered {
            result
        } else {
            STEP.scope((job.id.clone(),step.id.clone()),async{tokio::select!{biased;_=cancel.cancelled()=>Err("Workflow interrompu".into()),value=execute(folders.clone(),&step,value,&job.folder)=>value}}).await
        };
        if let Ok(runs) = registry().lock() {
            if let Some(run) = runs.get(&job.id) {
                job.effects = run.job.effects.clone()
            }
        }
        match outcome {
            Ok(mut result) => {
                if let Some(e) = job.effects.iter().rev().find(|e| e.step == step.id) {
                    result["job_folder"] = json!(e.folder)
                }
                if serde_json::to_vec(&result).map_or(true, |v| v.len() > 300_000) {
                    job.status = "failed".into();
                    job.error =
                        Some("Résultat trop volumineux ; utiliser les modes compacts".into());
                    break;
                }
                if serde_json::to_vec(&job.results).map_or(4_000_001, |v| v.len())
                    + serde_json::to_vec(&result).map_or(300_001, |v| v.len())
                    > 4_000_000
                {
                    job.status = "failed".into();
                    job.error =
                        Some("Résultats cumulés limités à 4 Mo : fractionner le workflow".into());
                    break;
                }
                job.results[&step.id] = result;
                job.completed += 1;
            }
            Err(error) => {
                if cancel.is_cancelled() {
                    job.status = if reason.load(Ordering::SeqCst) == 2 {
                        "cancelled"
                    } else {
                        "paused"
                    }
                    .into()
                } else {
                    job.status = "failed".into();
                    job.error = Some(format!("{} : {error}", step.id))
                }
                break;
            }
        }
        job.updated = state::now();
        if let Err(e) = persist(&job) {
            job.status = "failed".into();
            job.error = Some(format!("Journal indisponible : {e}"));
            break;
        }
        if let Ok(mut runs) = registry().lock() {
            if let Some(run) = runs.get_mut(&job.id) {
                run.job = job.clone()
            }
        }
    }
    if job.completed == job.steps.len() {
        job.status = "completed".into()
    }
    job.updated = state::now();
    let _ = persist(&job);
    if let Ok(mut runs) = registry().lock() {
        if let Some(run) = runs.get_mut(&job.id) {
            run.job = job
        }
    }
}
pub fn start(
    folders: Vec<SharedFolder>,
    args: StartArgs,
    parent: CancellationToken,
) -> Result<Value, String> {
    if args.steps.is_empty()
        || args.steps.len() > 50
        || args.name.len() > 120
        || serde_json::to_vec(&args.steps)
            .map_err(|e| e.to_string())?
            .len()
            > 1_000_000
    {
        return Err(
            "Workflow : 1–50 étapes, 1 Mo de paramètres et nom de 120 octets maximum".into(),
        );
    }
    let mut ids = HashSet::new();
    for s in &args.steps {
        if s.id.is_empty()
            || s.id.len() > 50
            || !s
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || !ids.insert(&s.id)
        {
            return Err("Identifiants d’étapes uniques : lettres, chiffres, tirets".into());
        }
    }
    let folder = library::select_folder(&folders, &args.folder)?.id.clone();
    let roots = folders
        .iter()
        .map(|f| f.path.canonicalize().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let job = Job {
        id: uuid::Uuid::new_v4().simple().to_string(),
        name: args.name,
        folder,
        roots,
        steps: args.steps,
        status: "pending".into(),
        completed: 0,
        results: json!({}),
        error: None,
        updated: state::now(),
        undo_completed: 0,
        effects: vec![],
    };
    launch(folders, job, parent)
}
pub fn control(
    folders: Vec<SharedFolder>,
    args: ControlArgs,
    parent: CancellationToken,
) -> Result<Value, String> {
    state::valid_id(&args.id)?;
    if ["pause", "cancel"].contains(&args.action.as_str()) {
        let mut runs = registry().lock().map_err(|_| "Workflows occupés")?;
        let run = runs
            .get_mut(&args.id)
            .ok_or("Workflow inactif ; utiliser resume")?;
        authorized(&folders, &run.job)?;
        run.reason.store(
            if args.action == "cancel" { 2 } else { 1 },
            Ordering::SeqCst,
        );
        run.cancel.cancel();
        return Ok(json!({"id":args.id,"requested":args.action}));
    }
    if args.action != "resume" {
        return Err("Action : resume, pause ou cancel".into());
    }
    let job = {
        let runs = registry().lock().map_err(|_| "Workflows occupés")?;
        if let Some(run) = runs.get(&args.id) {
            run.job.clone()
        } else {
            read(&args.id)?
        }
    };
    authorized(&folders, &job)?;
    if !["paused", "interrupted", "failed"].contains(&job.status.as_str()) {
        return Err(
            "Seuls les workflows suspendus, interrompus ou échoués peuvent reprendre".into(),
        );
    }
    for result in job.results.as_object().into_iter().flat_map(|m| m.values()) {
        if let (Some(folder), Some(path), Some(hash)) = (
            result["job_folder"].as_str(),
            result["path"].as_str(),
            result["sha256"].as_str(),
        ) {
            let path = crate::advanced::input(&folders, folder, path)?;
            if writing::hash(&fs::read(path).map_err(|e| e.to_string())?) != hash {
                return Err("Une sortie a changé ; reprise refusée. Créer un nouveau workflow pour intégrer vos changements.".into());
            }
        }
    }
    launch(folders, job, parent)
}
pub fn pause_all() {
    if let Ok(runs) = registry().lock() {
        for run in runs.values() {
            if run.job.status == "running" {
                run.reason.store(1, Ordering::SeqCst);
                run.cancel.cancel()
            }
        }
    }
}
pub fn undo(folders: &[SharedFolder], id: &str) -> Result<Value, String> {
    let mut job = {
        let runs = registry().lock().map_err(|_| "Workflows occupés")?;
        if let Some(r) = runs.get(id) {
            r.job.clone()
        } else {
            read(id)?
        }
    };
    authorized(folders, &job)?;
    if !["completed", "failed", "paused", "cancelled", "undo_partial"]
        .contains(&job.status.as_str())
    {
        return Err("Arrêter le workflow avant d’annuler ses fichiers".into());
    }
    let files: Vec<_> = job.effects.iter().rev().cloned().collect();
    let mut paths = HashSet::new();
    for e in &files {
        if !paths.insert((e.folder.clone(), e.path.clone())) {
            return Err(
                "Plusieurs écritures sur le même fichier : utiliser l’historique individuel".into(),
            );
        }
    }
    let remaining: Vec<_> = files.into_iter().skip(job.undo_completed).collect();
    for e in &remaining {
        effect_state(folders, e)?;
    }
    for e in remaining {
        let outcome = if !effect_state(folders, &e)? {
            Ok(json!({"not_applied":true}))
        } else if let Some(version) = &e.previous_version {
            crate::history::restore(folders, &e.folder, version, &e.sha256)
        } else {
            writing::remove_created(folders, &e.folder, &e.path, &e.sha256)
        };
        if let Err(error) = outcome {
            job.status = "undo_partial".into();
            job.error = Some(error.clone());
            persist(&job)?;
            registry()
                .lock()
                .map_err(|_| "Workflows occupés")?
                .remove(id);
            return Err(format!("Annulation partielle enregistrée : {error}"));
        }
        job.undo_completed += 1;
        persist(&job)?;
    }
    job.status = "undone".into();
    job.updated = state::now();
    persist(&job)?;
    registry()
        .lock()
        .map_err(|_| "Workflows occupés")?
        .remove(id);
    Ok(summary(&job))
}
