use crate::{Geometry, RequestId};

use super::{StartRequestError, TerminalStartRequest};

#[test]
fn start_request_requires_a_distinct_create_request_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    let geometry = Geometry::new(80, 24, 9, 18)?;
    assert_eq!(
        TerminalStartRequest::new(request_id.clone(), request_id, geometry),
        Err(StartRequestError::RequestIdentityMatchesCreate)
    );

    let create_request_id = RequestId::parse("22222222222222222222222222222222")?;
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    assert!(TerminalStartRequest::new(request_id, create_request_id, geometry).is_ok());
    Ok(())
}
