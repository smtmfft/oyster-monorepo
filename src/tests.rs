// TODO: tests have to be run one by one currently

#[cfg(test)]
pub mod serverless_executor_test {
    use std::collections::HashSet;

    use actix_web::body::MessageBody;
    use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
    use actix_web::web::Bytes;
    use actix_web::{http, test, web, App, Error};
    use ethers::providers::{Provider, Ws};
    use ethers::types::Address;
    use k256::ecdsa::SigningKey;
    use rand::rngs::OsRng;
    use serde_json::json;

    use crate::cgroups::Cgroups;
    use crate::node_handler::{deregister_enclave, index, inject_key, register_enclave};
    use crate::utils::AppState;

    // Testnet or Local blockchain (Hardhat) configurations
    const CHAIN_ID: u64 = 31337;
    const HTTP_RPC_URL: &str = "http://";
    const WS_URL: &str = "ws://";
    const EXECUTORS_CONTRACT_ADDR: &str = "0x";
    const JOBS_CONTRACT_ADDR: &str = "0x";
    const WALLET_PRIVATE_KEY: &str = "0x";
    const REGISTER_ATTESTATION: &str = "0x";
    const REGISTER_PCR_0: &str = "0x";
    const REGISTER_PCR_1: &str = "0x";
    const REGISTER_PCR_2: &str = "0x";
    const REGISTER_TIMESTAMP: usize = 2160;
    const REGISTER_STAKE_AMOUNT: usize = 100;

    fn new_app(
        ws_client: Provider<Ws>,
    ) -> App<
        impl ServiceFactory<
            ServiceRequest,
            Response = ServiceResponse<impl MessageBody + std::fmt::Debug>,
            Config = (),
            InitError = (),
            Error = Error,
        >,
    > {
        let signer = SigningKey::random(&mut OsRng);
        let signer_verifier_key: [u8; 64] =
            signer.verifying_key().to_encoded_point(false).to_bytes()[1..]
                .try_into()
                .unwrap();

        App::new()
            .app_data(web::Data::new(AppState {
                job_capacity: 20,
                cgroups: Cgroups::new().unwrap().into(),
                registered: false.into(),
                common_chain_id: CHAIN_ID,
                http_rpc_url: HTTP_RPC_URL.to_owned(),
                executors_contract_addr: EXECUTORS_CONTRACT_ADDR.parse::<Address>().unwrap(),
                executors_contract_object: None.into(),
                jobs_contract_addr: JOBS_CONTRACT_ADDR.parse::<Address>().unwrap(),
                jobs_contract_object: None.into(),
                // REPLACE RPC URL IN "workerd.rs" TO "https://sepolia-rollup.arbitrum.io/rpc"
                code_contract_addr: "0x44fe06d2940b8782a0a9a9ffd09c65852c0156b1".to_owned(),
                web_socket_client: ws_client,
                enclave_signer_key: signer,
                enclave_pub_key: Bytes::copy_from_slice(&signer_verifier_key),
                workerd_runtime_path: "./runtime/".to_owned(),
                job_requests_running: HashSet::new().into(),
                execution_buffer_time: 10,
            }))
            .service(index)
            .service(inject_key)
            .service(register_enclave)
            .service(deregister_enclave)
    }

    #[actix_web::test]
    async fn inject_key_test() {
        let app = test::init_service(new_app(Provider::<Ws>::connect(WS_URL).await.unwrap())).await;

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": "0x32255"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Failed to hex decode the key into 32 bytes: Odd number of digits"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": "0x322c322c322c332c352c35"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Failed to hex decode the key into 32 bytes: Invalid string length"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Invalid secret key provided: signature error"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": WALLET_PRIVATE_KEY
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Secret key injected successfully"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": WALLET_PRIVATE_KEY
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Secret key has already been injected"
        );
    }

    #[actix_web::test]
    async fn register_enclave_test() {
        let app = test::init_service(new_app(Provider::<Ws>::connect(WS_URL).await.unwrap())).await;

        let req = test::TestRequest::post()
            .uri("/register")
            .set_json(&json!({
                "attestation": REGISTER_ATTESTATION,
                "pcr_0": REGISTER_PCR_0,
                "pcr_1": REGISTER_PCR_1,
                "pcr_2": REGISTER_PCR_2,
                "timestamp": REGISTER_TIMESTAMP,
                "stake_amount": REGISTER_STAKE_AMOUNT
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Operator secret key not injected yet!"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": WALLET_PRIVATE_KEY
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Secret key injected successfully"
        );

        let req = test::TestRequest::post()
            .uri("/register")
            .set_json(&json!({
                "attestation": "0x32255",
                "pcr_0": "0x",
                "pcr_1": "0x",
                "pcr_2": "0x",
                "timestamp": 2160,
                "stake_amount": 100
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Invalid attestation hex string"
        );

        let req = test::TestRequest::post()
            .uri("/register")
            .set_json(&json!({
                "attestation": REGISTER_ATTESTATION,
                "pcr_0": REGISTER_PCR_0,
                "pcr_1": REGISTER_PCR_1,
                "pcr_2": REGISTER_PCR_2,
                "timestamp": REGISTER_TIMESTAMP,
                "stake_amount": REGISTER_STAKE_AMOUNT
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert!(resp
            .into_body()
            .try_into_bytes()
            .unwrap()
            .starts_with("Enclave Node successfully registered on the common chain".as_bytes()));

        let req = test::TestRequest::post()
            .uri("/register")
            .set_json(&json!({
                "attestation": REGISTER_ATTESTATION,
                "pcr_0": REGISTER_PCR_0,
                "pcr_1": REGISTER_PCR_1,
                "pcr_2": REGISTER_PCR_2,
                "timestamp": REGISTER_TIMESTAMP,
                "stake_amount": REGISTER_STAKE_AMOUNT
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave node is already registered!"
        );
    }

    #[actix_web::test]
    async fn deregister_enclave_test() {
        let app = test::init_service(new_app(Provider::<Ws>::connect(WS_URL).await.unwrap())).await;

        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Operator secret key not injected yet!"
        );

        let req = test::TestRequest::post()
            .uri("/inject-key")
            .set_json(&json!({
                "operator_secret": WALLET_PRIVATE_KEY
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Secret key injected successfully"
        );

        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave not registered yet!"
        );

        let req = test::TestRequest::post()
            .uri("/register")
            .set_json(&json!({
                "attestation": REGISTER_ATTESTATION,
                "pcr_0": REGISTER_PCR_0,
                "pcr_1": REGISTER_PCR_1,
                "pcr_2": REGISTER_PCR_2,
                "timestamp": REGISTER_TIMESTAMP,
                "stake_amount": REGISTER_STAKE_AMOUNT
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert!(resp
            .into_body()
            .try_into_bytes()
            .unwrap()
            .starts_with("Enclave Node successfully registered on the common chain".as_bytes()));

        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert!(resp.into_body().try_into_bytes().unwrap().starts_with(
            "Enclave Node successfully deregistered from the common chain".as_bytes()
        ));

        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave not registered yet!"
        );
    }
}
