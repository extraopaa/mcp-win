# TEST FIXTURE ONLY. Never use this public private key on a real relay.
& 'C:\Program Files\Git\usr\bin\openssl.exe' req -x509 -newkey rsa:2048 -noenc -keyout tests/fixtures/relay-key.pem -out tests/fixtures/relay-cert.pem -days 3650 -subj '/CN=localhost' -addext 'subjectAltName=DNS:localhost'
