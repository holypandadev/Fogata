use std::net::IpAddr;
use std::str::FromStr;

use tokio::net::UdpSocket;
use tokio::time::{Duration, Instant};
use util::vnet::net::*;

use super::*;
use crate::relay::relay_none::*;

const STATIC_KEY: &str = "ABC";

#[tokio::test]
async fn test_allocation_lifetime_parsing() -> Result<()> {
    let lifetime = Lifetime(Duration::from_secs(5));

    let mut m = Message::new();
    let lifetime_duration = allocation_lifetime(&m);

    assert_eq!(
        lifetime_duration, DEFAULT_LIFETIME,
        "Allocation lifetime should be default time duration"
    );

    lifetime.add_to(&mut m)?;

    let lifetime_duration = allocation_lifetime(&m);
    assert_eq!(
        lifetime_duration, lifetime.0,
        "Expect lifetime_duration is {lifetime}, but {lifetime_duration:?}"
    );

    Ok(())
}

#[tokio::test]
async fn test_allocation_lifetime_overflow() -> Result<()> {
    let lifetime = Lifetime(MAXIMUM_ALLOCATION_LIFETIME * 2);

    let mut m2 = Message::new();
    lifetime.add_to(&mut m2)?;

    let lifetime_duration = allocation_lifetime(&m2);
    assert_eq!(
        lifetime_duration, DEFAULT_LIFETIME,
        "Expect lifetime_duration is {DEFAULT_LIFETIME:?}, but {lifetime_duration:?}"
    );

    Ok(())
}

struct TestAuthHandler;
impl AuthHandler for TestAuthHandler {
    fn auth_handle(&self, _username: &str, _realm: &str, _src_addr: SocketAddr) -> Result<Vec<u8>> {
        Ok(STATIC_KEY.as_bytes().to_vec())
    }
}

#[tokio::test]
async fn test_allocation_lifetime_deletion_zero_lifetime() -> Result<()> {
    //env_logger::init();

    let l = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);

    let allocation_manager = Arc::new(Manager::new(ManagerConfig {
        relay_addr_generator: Box::new(RelayAddressGeneratorNone {
            address: "0.0.0.0".to_owned(),
            net: Arc::new(Net::new(None)),
        }),
        alloc_close_notify: None,
    }));

    let socket = SocketAddr::new(IpAddr::from_str("127.0.0.1")?, 5000);

    let mut r = Request::new(l, socket, allocation_manager, Arc::new(TestAuthHandler {}));

    {
        let mut nonces = r.nonces.lock().await;
        nonces.insert(STATIC_KEY.to_owned(), Instant::now());
    }

    let five_tuple = FiveTuple {
        src_addr: r.src_addr,
        dst_addr: r.conn.local_addr()?,
        protocol: PROTO_UDP,
    };

    r.allocation_manager
        .create_allocation(
            five_tuple,
            Arc::clone(&r.conn),
            0,
            Duration::from_secs(3600),
            TextAttribute::new(ATTR_USERNAME, "user".into()),
            true,
        )
        .await?;
    assert!(r
        .allocation_manager
        .get_allocation(&five_tuple)
        .await
        .is_some());

    let mut m = Message::new();
    Lifetime::default().add_to(&mut m)?;
    MessageIntegrity(STATIC_KEY.as_bytes().to_vec()).add_to(&mut m)?;
    Nonce::new(ATTR_NONCE, STATIC_KEY.to_owned()).add_to(&mut m)?;
    Realm::new(ATTR_REALM, STATIC_KEY.to_owned()).add_to(&mut m)?;
    Username::new(ATTR_USERNAME, STATIC_KEY.to_owned()).add_to(&mut m)?;

    r.handle_refresh_request(&m).await?;
    assert!(r
        .allocation_manager
        .get_allocation(&five_tuple)
        .await
        .is_none());

    Ok(())
}

// Fogata: requests without credentials, each answered with a nonce never used, do not grow
// the nonces kept without end; those past their lifetime go, then the oldest.
#[tokio::test]
async fn test_nonces_never_used_are_bounded() -> Result<()> {
    let l = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let allocation_manager = Arc::new(Manager::new(ManagerConfig {
        relay_addr_generator: Box::new(RelayAddressGeneratorNone {
            address: "0.0.0.0".to_owned(),
            net: Arc::new(Net::new(None)),
        }),
        alloc_close_notify: None,
    }));
    // Nobody listens there: the answers are lost, as a flood's would be.
    let socket = SocketAddr::new(IpAddr::from_str("127.0.0.1")?, 9);
    let mut r = Request::new(l, socket, allocation_manager, Arc::new(TestAuthHandler {}));
    let mut m = Message::new();
    m.build(&[
        Box::new(TransactionId::new()),
        Box::new(MessageType::new(METHOD_ALLOCATE, CLASS_REQUEST)),
    ])?;
    for _ in 0..5000 {
        assert!(r.authenticate_request(&m, METHOD_ALLOCATE).await?.is_none());
    }
    assert!(r.nonces.lock().await.len() <= MAX_NONCES);

    // Kept past their lifetime, they all go once a new one is made (unless the clock
    // started less than that long ago).
    let Some(made) = Instant::now().checked_sub(NONCE_LIFETIME + Duration::from_secs(1)) else {
        return Ok(());
    };
    for nonce in r.nonces.lock().await.values_mut() {
        *nonce = made;
    }
    assert!(r.authenticate_request(&m, METHOD_ALLOCATE).await?.is_none());
    assert_eq!(r.nonces.lock().await.len(), 1);
    Ok(())
}
