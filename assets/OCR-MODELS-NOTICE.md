# OCR models

Unmodified French and English LSTM recognition models from the Tesseract OCR
project, tessdata_fast 4.1.0. License: Apache-2.0.

Sources: https://github.com/tesseract-ocr/tessdata_fast/tree/4.1.0
License: https://github.com/tesseract-ocr/tessdata_fast/blob/4.1.0/LICENSE

Tesseract 5.5.0 is built from pinned upstream sources, statically linked with
Leptonica 1.85.0, libpng 1.6.44 and zlib 1.3.1. This build supports PNG input;
the Rust image layer converts other supported input formats to PNG first.
No curl, libarchive, training tools, legacy engine or third-party installer
is shipped. It performs no network requests.

Hashes and model URLs: scripts/fetch-engines.ps1.
Source URLs, hashes and build options: scripts/build-tesseract.ps1.
Licenses are included under engines/tesseract/licenses/ (Apache-2.0 for the
engine and models; BSD-style Leptonica, libpng and zlib licenses for dependencies).
