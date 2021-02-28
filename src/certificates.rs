use {
    rustls::{
        internal::pemfile::{certs, pkcs8_private_keys},
        sign::{CertifiedKey, RSASigningKey},
        ResolvesServerCert,
    },
    std::{
        ffi::OsStr,
        fmt::{Display, Formatter},
        fs::File,
        io::BufReader,
        path::Path,
        sync::Arc,
    },
    webpki::DNSNameRef,
};

/// A struct that holds all loaded certificates and the respective domain
/// names.
pub(crate) struct CertStore {
    // use a Vec of pairs instead of a HashMap because order matters
    certs: Vec<(String, CertifiedKey)>,
}

static CERT_FILE_NAME: &str = "cert.pem";
static KEY_FILE_NAME: &str = "key.rsa";

#[derive(Debug)]
pub enum CertLoadError {
    /// could not access the certificate root directory
    NoReadCertDir,
    /// the specified domain name cannot be processed correctly
    BadDomain(String),
    /// the key file for the specified domain is bad (e.g. does not contain a
    /// key or is invalid)
    BadKey(String),
    /// the certificate file for the specified domain is bad (e.g. invalid)
    BadCert(String),
    /// the key file for the specified domain is missing (but a certificate
    /// file was present)
    MissingKey(String),
    /// the certificate file for the specified domain is missing (but a key
    /// file was present)
    MissingCert(String),
    /// neither a key file nor a certificate file were present for the given
    /// domain (but a folder was present)
    EmptyDomain(String),
}

impl Display for CertLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoReadCertDir => write!(f, "Could not read from certificate directory."),
            Self::BadDomain(domain) if !domain.is_ascii() => write!(
                f,
                "The domain name {} cannot be processed, it must be punycoded.",
                domain
            ),
            Self::BadDomain(domain) => write!(f, "The domain name {} cannot be processed.", domain),
            Self::BadKey(domain) => write!(f, "The key file for {} is malformed.", domain),
            Self::BadCert(domain) => write!(f, "The certificate file for {} is malformed.", domain),
            Self::MissingKey(domain) => write!(f, "The key file for {} is missing.", domain),
            Self::MissingCert(domain) => {
                write!(f, "The certificate file for {} is missing.", domain)
            }
            Self::EmptyDomain(domain) => write!(
                f,
                "A folder for {} exists, but there is no certificate or key file.",
                domain
            ),
        }
    }
}

impl std::error::Error for CertLoadError {}

fn load_domain(certs_dir: &Path, domain: String) -> Result<CertifiedKey, CertLoadError> {
    let mut path = certs_dir.to_path_buf();
    path.push(&domain);
    // load certificate from file
    path.push(CERT_FILE_NAME);
    if !path.is_file() {
        return Err(if !path.with_file_name(KEY_FILE_NAME).is_file() {
            CertLoadError::EmptyDomain(domain)
        } else {
            CertLoadError::MissingCert(domain)
        });
    }

    let cert_chain = match certs(&mut BufReader::new(File::open(&path).unwrap())) {
        Ok(cert) => cert,
        Err(_) => return Err(CertLoadError::BadCert(domain)),
    };

    // load key from file
    path.set_file_name(KEY_FILE_NAME);
    if !path.is_file() {
        return Err(CertLoadError::MissingKey(domain));
    }
    let key = match pkcs8_private_keys(&mut BufReader::new(File::open(&path).unwrap())) {
        Ok(mut keys) if !keys.is_empty() => keys.remove(0),
        _ => return Err(CertLoadError::BadKey(domain)),
    };

    // transform key to correct format
    let key = match RSASigningKey::new(&key) {
        Ok(key) => key,
        Err(_) => return Err(CertLoadError::BadKey(domain)),
    };
    Ok(CertifiedKey::new(cert_chain, Arc::new(Box::new(key))))
}

impl CertStore {
    /// Load certificates from a certificate directory.
    /// Certificates should be stored in a folder for each hostname, for example
    /// the certificate and key for `example.com` should be in the files
    /// `certs_dir/example.com/{cert.pem,key.rsa}` respectively.
    ///
    /// If there are `cert.pem` and `key.rsa` directly in certs_dir, these will be
    /// loaded as default certificates.
    pub fn load_from(certs_dir: &Path) -> Result<Self, CertLoadError> {
        // load all certificates from directories
        let mut certs = vec![];

        // try to load fallback certificate and key
        match load_domain(certs_dir, ".".to_string()) {
            Err(CertLoadError::EmptyDomain(_)) => { /* there are no fallback keys */ }
            Err(CertLoadError::NoReadCertDir) => unreachable!(),
            Err(CertLoadError::BadDomain(_)) => unreachable!(),
            Err(CertLoadError::BadKey(_)) => {
                return Err(CertLoadError::BadKey("fallback".to_string()))
            }
            Err(CertLoadError::BadCert(_)) => {
                return Err(CertLoadError::BadCert("fallback".to_string()))
            }
            Err(CertLoadError::MissingKey(_)) => {
                return Err(CertLoadError::MissingKey("fallback".to_string()))
            }
            Err(CertLoadError::MissingCert(_)) => {
                return Err(CertLoadError::MissingCert("fallback".to_string()))
            }
            // if there are files, just push them because there is no domain
            // name to check against
            Ok(key) => certs.push((String::new(), key)),
        }

        for file in certs_dir
            .read_dir()
            .or(Err(CertLoadError::NoReadCertDir))?
            .filter_map(Result::ok)
            .filter(|x| x.path().is_dir())
        {
            let path = file.path();
            let filename = path
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap()
                .to_string();

            let dns_name = match DNSNameRef::try_from_ascii_str(&filename) {
                Ok(name) => name,
                Err(_) => return Err(CertLoadError::BadDomain(filename)),
            };

            let key = load_domain(certs_dir, filename.clone())?;
            if key.cross_check_end_entity_cert(Some(dns_name)).is_err() {
                return Err(CertLoadError::BadCert(filename));
            }

            certs.push((filename, key));
        }

        certs.sort_unstable_by(|(a, _), (b, _)| {
            // try to match as many as possible. If one is a substring of the other,
            // the `zip` will make them look equal and make the length decide.
            for (a_part, b_part) in a.split('.').rev().zip(b.split('.').rev()) {
                if a_part != b_part {
                    return a_part.cmp(b_part);
                }
            }
            // longer domains first
            a.len().cmp(&b.len()).reverse()
        });

        Ok(Self { certs })
    }
}

impl ResolvesServerCert for CertStore {
    fn resolve(&self, client_hello: rustls::ClientHello<'_>) -> Option<CertifiedKey> {
        if let Some(name) = client_hello.server_name() {
            let name: &str = name.into();
            self.certs
                .iter()
                .find(|(s, _)| name.ends_with(s))
                .map(|(_, k)| k)
                .cloned()
        } else {
            // This kind of resolver requires SNI
            None
        }
    }
}
