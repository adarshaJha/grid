// Copyright 2019 Bitwise IO, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod error;
mod route_handler;

use std::sync::mpsc;
use std::thread;

use actix_web::{http::Method, server, App};

pub use crate::rest_api::error::RestApiError;
use crate::rest_api::route_handler::index;

fn create_app() -> App {
    App::new().resource("/", |r| r.method(Method::GET).f(index))
}

pub struct RestApiShutdownHandle {
    do_shutdown: Box<dyn Fn() -> Result<(), RestApiError> + Send>,
}

impl RestApiShutdownHandle {
    pub fn shutdown(&self) -> Result<(), RestApiError> {
        (*self.do_shutdown)()
    }
}

pub fn run(
    bind_url: &str,
) -> Result<
    (
        RestApiShutdownHandle,
        thread::JoinHandle<Result<(), RestApiError>>,
    ),
    RestApiError,
> {
    let (tx, rx) = mpsc::channel();
    let bind_url = bind_url.to_owned();
    let join_handle = thread::Builder::new()
        .name("GridRestApi".into())
        .spawn(move || {
            let sys = actix::System::new("Grid-Rest-API");

            info!("Starting Rest API at {}", &bind_url);
            let addr = server::new(create_app)
                .bind(bind_url)?
                .disable_signals()
                .system_exit()
                .start();

            tx.send(addr).map_err(|err| {
                RestApiError::StartUpError(format!("Unable to send Server Addr: {}", err))
            })?;

            sys.run();

            info!("Rest API terminating");

            Ok(())
        })?;

    let addr = rx.recv().map_err(|err| {
        RestApiError::StartUpError(format!("Unable to receive Server Addr: {}", err))
    })?;

    let do_shutdown = Box::new(move || {
        debug!("Shutting down Rest API");
        addr.do_send(server::StopServer { graceful: true });
        debug!("Graceful signal sent to Rest API");

        Ok(())
    });

    Ok((RestApiShutdownHandle { do_shutdown }, join_handle))
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::test::TestServer;
    use actix_web::HttpMessage;
    use std::str;

    #[test]
    fn index_test() {
        let mut srv = TestServer::new(|app| app.handler(index));

        let req = srv.get().finish().unwrap();
        let resp = srv.execute(req.send()).unwrap();
        assert!(resp.status().is_success());

        let body_bytes = srv.execute(resp.body()).unwrap();
        let body_str = str::from_utf8(&body_bytes).unwrap();
        assert_eq!(body_str, "Hello world!");
    }
}
