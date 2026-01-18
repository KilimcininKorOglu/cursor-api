mod session;

use crate::{
    app::{lazy::chat_url, model::ExtToken},
    common::{
        client::{AiServiceRequest, build_client_request},
        utils::new_uuid_v4,
    },
};
use session::{Session, SessionSink, SessionStream};

pub enum Tendency<N, O> {
    Start(N),
    Continue(O),
}

pub async fn start(
    data: Vec<u8>,
    ext_token: &ExtToken,
    use_pri: bool,
) -> Result<Session, reqwest::Error> {
    let (sink, body) = SessionSink::new();
    let req = build_client_request(AiServiceRequest {
        ext_token,
        fs_client_key: None,
        url: chat_url(use_pri),
        stream: true,
        compressed: true,
        trace_id: new_uuid_v4(),
        use_pri,
        cookie: None,
    });
    let res = req.body(body).send().await?;
    __unwrap!(sink.send(data.into()).await);
    let stream = SessionStream::new(res);

    Ok(Session { stream, sink })
}
