// TODO: tests have to be run one by one currently

#[cfg(test)]
pub mod serverless_executor_test {
    use std::collections::HashSet;

    use actix_web::body::MessageBody;
    use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
    use actix_web::web::{Bytes, Data};
    use actix_web::{http, test, App, Error};
    use ethers::providers::{Provider, Ws};
    use ethers::types::{Address, U256};
    use k256::ecdsa::SigningKey;
    use rand::rngs::OsRng;
    use serde_json::json;
    use tokio::sync::mpsc::channel;

    use crate::cgroups::Cgroups;
    use crate::job_handler::execute_job;
    use crate::node_handler::{deregister_enclave, index, inject_key, register_enclave};
    use crate::utils::{AppState, JobResponse};

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

    // Generate test app state
    async fn generate_app_state(rpc: &str) -> Data<AppState> {
        // Initialize random 'secp256k1' signing key for the enclave
        let signer = SigningKey::random(&mut OsRng);
        let signer_verifier_key: [u8; 64] =
            signer.verifying_key().to_encoded_point(false).to_bytes()[1..]
                .try_into()
                .unwrap();

        Data::new(AppState {
            job_capacity: 20,
            cgroups: Cgroups::new().unwrap().into(),
            registered: false.into(),
            common_chain_id: CHAIN_ID,
            http_rpc_url: rpc.to_owned(),
            executors_contract_addr: EXECUTORS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            executors_contract_object: None.into(),
            jobs_contract_addr: JOBS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            jobs_contract_object: None.into(),
            // REPLACE RPC URL IN "workerd.rs" TO "https://sepolia-rollup.arbitrum.io/rpc"
            code_contract_addr: "0x44fe06d2940b8782a0a9a9ffd09c65852c0156b1".to_owned(),
            web_socket_client: Provider::<Ws>::connect(WS_URL).await.unwrap(),
            enclave_signer_key: signer,
            enclave_pub_key: Bytes::copy_from_slice(&signer_verifier_key),
            workerd_runtime_path: "./runtime/".to_owned(),
            job_requests_running: HashSet::new().into(),
            execution_buffer_time: 10,
        })
    }

    // Return the actix server with the provided app state
    fn new_app(
        app_state: Data<AppState>,
    ) -> App<
        impl ServiceFactory<
            ServiceRequest,
            Response = ServiceResponse<impl MessageBody + std::fmt::Debug>,
            Config = (),
            InitError = (),
            Error = Error,
        >,
    > {
        App::new()
            .app_data(app_state)
            .service(index)
            .service(inject_key)
            .service(register_enclave)
            .service(deregister_enclave)
    }

    #[actix_web::test]
    // Test the various response cases for the 'inject_key' endpoint
    async fn inject_key_test() {
        let app = test::init_service(new_app(generate_app_state(HTTP_RPC_URL).await)).await;

        // Inject invalid hex private key string
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

        // Inject invalid length private key
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

        // Inject invalid private(signing) key
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

        // Inject a valid private key
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

        // Inject the valid private key again
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
    // Test the various response cases for the 'register_enclave' endpoint
    async fn register_enclave_test() {
        let app = test::init_service(new_app(generate_app_state(HTTP_RPC_URL).await)).await;

        // Register the executor without injecting the operator's private key
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

        // Inject a valid private key into the enclave
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

        // Register the enclave with an invalid attestation hex string
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

        // Register the enclave with valid data points
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

        // Register the enclave again before deregistering
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
    // Test the various response cases for the 'deregister_enclave' endpoint
    async fn test_deregister_enclave() {
        let app = test::init_service(new_app(generate_app_state(HTTP_RPC_URL).await)).await;

        // Deregister the enclave without even injecting the private key
        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Operator secret key not injected yet!"
        );

        // Inject a valid private key
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

        // Deregister the enclave before even registering it
        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave not registered yet!"
        );

        // Register the enclave with valid data points
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

        // Deregister the enclave
        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert!(resp.into_body().try_into_bytes().unwrap().starts_with(
            "Enclave Node successfully deregistered from the common chain".as_bytes()
        ));

        // Deregister the enclave again before registering it
        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave not registered yet!"
        );
    }

    // Execute a job request using 'job_handler' and return the response
    async fn job_handler_unit_test(
        job_id: U256,
        req_chain_id: U256,
        code_hash: String,
        code_inputs: Bytes,
        user_deadline: u64,
    ) -> Option<JobResponse> {
        let app_state = generate_app_state("https://sepolia-rollup.arbitrum.io/rpc").await;
        let (tx, mut rx) = channel::<JobResponse>(100);

        tokio::spawn(async move {
            execute_job(
                job_id,
                req_chain_id,
                code_hash,
                code_inputs,
                user_deadline,
                app_state,
                tx,
            )
            .await;
        });

        while let Some(job_response) = rx.recv().await {
            return Some(job_response);
        }

        return None;
    }

    #[actix_web::test]
    // Test a valid job request with different inputs and verify the response from the 'job_handler'
    async fn valid_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,5");

        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 20
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,2,5");

        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 600
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,2,2,3,5,5");
    }

    #[actix_web::test]
    // Test a valid job request with invalid input and verify the response from the 'job_handler'
    async fn invalid_input_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({})).unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(
            execution_response.output,
            "Please provide a valid integer as input in the format{'num':10}"
        );
    }

    #[actix_web::test]
    // Test '1' error code job requests and verify the response from the 'job_handler'
    async fn invalid_transaction_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // Given transaction hash doesn't belong to the expected smart contract
        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0xfed8ab36cc27831836f6dcb7291049158b4d8df31c0ffb05a3d36ba6555e29d7".to_owned(),
            code_input_bytes.clone().into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 1);
        assert_eq!(execution_response.output, "");

        // Given transaction hash doesn't exist in the expected rpc network
        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x37b0b2d9dd58d9130781fc914da456c16ec403010e8d4c27b0ea4657a24c8546".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 1);
        assert_eq!(execution_response.output, "");
    }

    #[actix_web::test]
    // Test '3' error code job requests and verify the response from the 'job_handler'
    async fn invalid_code_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // Code corresponding to the provided transaction hash has a syntax error
        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x96179f60fd7917c04ad9da6dd64690a1a960f39b50029d07919bf2628f5e7fe5".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 3);
        assert_eq!(execution_response.output, "");
    }

    #[actix_web::test]
    // Test '4' error code job requests and verify the response from the 'job_handler'
    async fn deadline_timeout_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // User code didn't return a response in the expected period
        let job_response = job_handler_unit_test(
            1.into(),
            1.into(),
            "0x9c641b535e5586200d0f2fd81f05a39436c0d9dd35530e9fb3ca18352c3ba111".to_owned(),
            code_input_bytes.into(),
            10,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.req_chain_id, 1.into());
        assert_eq!(execution_response.error_code, 4);
        assert_eq!(execution_response.output, "");
    }
}
