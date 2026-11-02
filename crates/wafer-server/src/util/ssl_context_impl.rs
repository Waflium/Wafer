use std::sync::OnceLock;

use tokio_rustls::rustls::client::{ClientSessionMemoryCache, ClientSessionStore, Tls12ClientSessionValue, Tls13ClientSessionValue};
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::crypto::cipher::{AeadKey, InboundOpaqueMessage, InboundPlainMessage, KeyBlockShape, MessageDecrypter, MessageEncrypter, OutboundOpaqueMessage, OutboundPlainMessage, Tls12AeadAlgorithm, UnsupportedOperationError};
use tokio_rustls::rustls::crypto::{ActiveKeyExchange, CipherSuiteCommon, CryptoProvider, KeyExchangeAlgorithm, SharedSecret, SupportedKxGroup, ring};
use tokio_rustls::rustls::{CipherSuite, ConnectionTrafficSecrets, Error, NamedGroup, SignatureScheme, SupportedCipherSuite, Tls12CipherSuite};

pub const SIGNATURE_SCHEMES: [u16; 15] = [0x0403, 0x0503, 0x0603, 0x0807, 0x0808, 0x0804, 0x0805, 0x0806, 0x0809, 0x080a, 0x080b, 0x0401, 0x0501, 0x0601, 0x0402];
pub const SIGNATURE_SCHEMES_CERT: [u16; 18] = [0x0403, 0x0503, 0x0603, 0x0807, 0x0808, 0x0804, 0x0805, 0x0806, 0x0809, 0x080a, 0x080b, 0x0401, 0x0501, 0x0601, 0x0402, 0x0203, 0x0201, 0x0202];
pub const STATUS_REQUEST_V2: [u8; 9] = [0x00, 0x07, 0x02, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00];

const UNIMPLEMENTED_CIPHER_SUITES: [(u16, u16); 21] = [
    (0x009f, 0xc030),
    (0xccaa, 0xcca8),
    (0x00a3, 0xc030),
    (0x009e, 0xc02f),
    (0x00a2, 0xc02f),
    (0xc024, 0xc030),
    (0xc028, 0xc030),
    (0xc023, 0xc02f),
    (0xc027, 0xc02f),
    (0x006b, 0xc02f),
    (0x006a, 0xc02f),
    (0x0067, 0xc02f),
    (0x0040, 0xc02f),
    (0xc00a, 0xc02f),
    (0xc014, 0xc02f),
    (0xc009, 0xc02f),
    (0xc013, 0xc02f),
    (0x0039, 0xc02f),
    (0x0038, 0xc02f),
    (0x0033, 0xc02f),
    (0x0032, 0xc02f),
];

const IMPLEMENTED_CIPHER_SUITES: [u16; 9] = [0x1302, 0x1301, 0x1303, 0xc02c, 0xc02b, 0xcca9, 0xc030, 0xcca8, 0xc02f];

const UNIMPLEMENTED_NAMED_GROUPS: [u16; 7] = [0x0019, 0x001e, 0x0100, 0x0101, 0x0102, 0x0103, 0x0104];

pub fn signature_schemes(schemes: &[u16]) -> Vec<SignatureScheme> {
    schemes.iter().map(|&scheme| SignatureScheme::from(scheme)).collect()
}

pub fn client_provider() -> CryptoProvider {
    static PROVIDER: OnceLock<CryptoProvider> = OnceLock::new();
    PROVIDER
        .get_or_init(|| {
            let ring = ring::default_provider();
            let find = |suite: u16| *ring.cipher_suites.iter().find(|candidate| u16::from(candidate.suite()) == suite).expect("a ring cipher suite");
            let mut cipher_suites: Vec<SupportedCipherSuite> = IMPLEMENTED_CIPHER_SUITES.iter().map(|&suite| find(suite)).collect();
            for (suite, like) in UNIMPLEMENTED_CIPHER_SUITES {
                let SupportedCipherSuite::Tls12(like) = find(like) else { unreachable!() };
                let unimplemented: &'static Tls12CipherSuite = Box::leak(Box::new(Tls12CipherSuite {
                    common: CipherSuiteCommon { suite: CipherSuite::from(suite), hash_provider: like.common.hash_provider, confidentiality_limit: like.common.confidentiality_limit },
                    prf_provider: like.prf_provider,
                    kx: KeyExchangeAlgorithm::ECDHE,
                    sign: &[],
                    aead_alg: &UnimplementedCipher,
                }));
                cipher_suites.push(SupportedCipherSuite::Tls12(unimplemented));
            }
            let mut kx_groups: Vec<&'static dyn SupportedKxGroup> = vec![&KeyShares, ring::kx_group::SECP256R1, ring::kx_group::SECP384R1];
            for group in UNIMPLEMENTED_NAMED_GROUPS {
                kx_groups.push(Box::leak(Box::new(UnimplementedGroup(NamedGroup::from(group)))));
            }
            CryptoProvider { cipher_suites, kx_groups, ..ring }
        })
        .clone()
}

#[derive(Debug)]
pub struct SslSessionContextImpl {
    cache: ClientSessionMemoryCache,
}

impl SslSessionContextImpl {
    pub fn new() -> SslSessionContextImpl {
        SslSessionContextImpl { cache: ClientSessionMemoryCache::new(SESSION_CACHE_SIZE) }
    }
}

const SESSION_CACHE_SIZE: usize = 20480;

impl ClientSessionStore for SslSessionContextImpl {
    fn set_kx_hint(&self, _server_name: ServerName<'static>, _group: NamedGroup) {}

    fn kx_hint(&self, _server_name: &ServerName<'_>) -> Option<NamedGroup> {
        None
    }

    fn set_tls12_session(&self, server_name: ServerName<'static>, value: Tls12ClientSessionValue) {
        self.cache.set_tls12_session(server_name, value)
    }

    fn tls12_session(&self, server_name: &ServerName<'_>) -> Option<Tls12ClientSessionValue> {
        self.cache.tls12_session(server_name)
    }

    fn remove_tls12_session(&self, server_name: &ServerName<'static>) {
        self.cache.remove_tls12_session(server_name)
    }

    fn insert_tls13_ticket(&self, server_name: ServerName<'static>, value: Tls13ClientSessionValue) {
        self.cache.insert_tls13_ticket(server_name, value)
    }

    fn take_tls13_ticket(&self, server_name: &ServerName<'static>) -> Option<Tls13ClientSessionValue> {
        self.cache.take_tls13_ticket(server_name)
    }
}

#[derive(Debug)]
struct KeyShares;

impl SupportedKxGroup for KeyShares {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        Ok(Box::new(ActiveKeyShares { x25519: ring::kx_group::X25519.start()?, secp256r1: ring::kx_group::SECP256R1.start()? }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

struct ActiveKeyShares {
    x25519: Box<dyn ActiveKeyExchange>,
    secp256r1: Box<dyn ActiveKeyExchange>,
}

impl ActiveKeyExchange for ActiveKeyShares {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, Error> {
        self.x25519.complete(peer_pub_key)
    }

    fn hybrid_component(&self) -> Option<(NamedGroup, &[u8])> {
        Some((NamedGroup::secp256r1, self.secp256r1.pub_key()))
    }

    fn complete_hybrid_component(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, Error> {
        self.secp256r1.complete(peer_pub_key)
    }

    fn pub_key(&self) -> &[u8] {
        self.x25519.pub_key()
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

#[derive(Debug)]
struct UnimplementedGroup(NamedGroup);

impl SupportedKxGroup for UnimplementedGroup {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        Err(Error::General(format!("unimplemented named group {:?}", self.0)))
    }

    fn name(&self) -> NamedGroup {
        self.0
    }
}

struct UnimplementedCipher;

impl Tls12AeadAlgorithm for UnimplementedCipher {
    fn encrypter(&self, _key: AeadKey, _iv: &[u8], _extra: &[u8]) -> Box<dyn MessageEncrypter> {
        Box::new(UnimplementedCipher)
    }

    fn decrypter(&self, _key: AeadKey, _iv: &[u8]) -> Box<dyn MessageDecrypter> {
        Box::new(UnimplementedCipher)
    }

    fn key_block_shape(&self) -> KeyBlockShape {
        KeyBlockShape { enc_key_len: 16, fixed_iv_len: 4, explicit_nonce_len: 8 }
    }

    fn extract_keys(&self, _key: AeadKey, _iv: &[u8], _explicit: &[u8]) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        Err(UnsupportedOperationError)
    }
}

impl MessageEncrypter for UnimplementedCipher {
    fn encrypt(&mut self, _msg: OutboundPlainMessage<'_>, _seq: u64) -> Result<OutboundOpaqueMessage, Error> {
        Err(Error::EncryptError)
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len
    }
}

impl MessageDecrypter for UnimplementedCipher {
    fn decrypt<'a>(&mut self, _msg: InboundOpaqueMessage<'a>, _seq: u64) -> Result<InboundPlainMessage<'a>, Error> {
        Err(Error::DecryptError)
    }
}
