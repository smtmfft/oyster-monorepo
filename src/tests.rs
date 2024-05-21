// TODO: tests have to be run one by one currently
/* NOTE: To run an unit test 'test_name', hit the following commands on terminal ->
   1.    sudo ./cgroupv2_setup.sh
   2.    sudo echo && cargo test test name -- --nocapture &
*/

#[cfg(test)]
pub mod serverless_executor_test {
    use core::panic;
    use std::collections::HashSet;
    use std::str::FromStr;

    use actix_web::body::MessageBody;
    use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
    use actix_web::web::{Bytes, Data};
    use actix_web::{http, test, App, Error};
    use ethers::abi::{encode, Token};
    use ethers::providers::{Provider, StreamExt, Ws};
    use ethers::types::{Address, BigEndianHash, Log, H160, H256, U256};
    use ethers::utils::keccak256;
    use k256::ecdsa::SigningKey;
    use rand::rngs::OsRng;
    use serde_json::json;
    use tokio::sync::mpsc::channel;
    use tokio::time::{sleep, Duration};

    use crate::cgroups::Cgroups;
    use crate::event_handler::handle_event_logs;
    use crate::node_handler::{deregister_enclave, index, inject_key, register_enclave};
    use crate::utils::{AppState, JobResponse};

    // Testnet or Local blockchain (Hardhat) configurations
    const CHAIN_ID: u64 = 421614;
    const HTTP_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
    const WS_URL: &str = "wss://arbitrum-sepolia.infura.io/ws/v3/cd72f20b9fd544f8a5b8da706441e01c";
    const EXECUTORS_CONTRACT_ADDR: &str = "0xdec0719F26f3771D9E84Cf8694DAE79F3f2AbEbB";
    const JOBS_CONTRACT_ADDR: &str = "0xAc6Ae536203a3ec290ED4aA1d3137e6459f4A963";
    const CODE_CONTRACT_ADDR: &str = "0x44fe06d2940b8782a0a9a9ffd09c65852c0156b1";
    const WALLET_PRIVATE_KEY: &str =
        "0xa8b743563462eb4b943e3de02ce7fcdfde6ca255b2f6850f34d47c1a9824b2f8";
    const REGISTER_ATTESTATION: &str = "0xcfa7554f87ba13620037695d62a381a2d876b74c2e1b435584fe5c02c53393ac1c5cd5a8b6f92e866f9a65af751e0462cfa7554f87ba13620037695d62a381a2d8";
    const REGISTER_PCR_0: &str = "0xcfa7554f87ba13620037695d62a381a2d876b74c2e1b435584fe5c02c53393ac1c5cd5a8b6f92e866f9a65af751e0462";
    const REGISTER_PCR_1: &str = "0xbcdf05fefccaa8e55bf2c8d6dee9e79bbff31e34bf28a99aa19e6b29c37ee80b214a414b7607236edf26fcb78654e63f";
    const REGISTER_PCR_2: &str = "0x20caae8a6a69d9b1aecdf01a0b9c5f3eafd1f06cb51892bf47cef476935bfe77b5b75714b68a69146d650683a217c5b3";
    const REGISTER_TIMESTAMP: usize = 1722134849000;
    const REGISTER_STAKE_AMOUNT: usize = 100;

    // Generate test app state
    async fn generate_app_state() -> Data<AppState> {
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
            num_selected_executors: 1,
            common_chain_id: CHAIN_ID,
            http_rpc_url: HTTP_RPC_URL.to_owned(),
            http_rpc_client: None.into(),
            web_socket_client: Provider::<Ws>::connect(WS_URL).await.unwrap(),
            executors_contract_addr: EXECUTORS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            jobs_contract_addr: JOBS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            code_contract_addr: CODE_CONTRACT_ADDR.to_owned(),
            executor_operator_key: None.into(),
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
        let app = test::init_service(new_app(generate_app_state().await)).await;

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
            "Failed to hex decode the key into 32 bytes: OddLength"
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
            "Failed to hex decode the key into 32 bytes: InvalidStringLength"
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
            "Invalid secret key provided: EcdsaError(signature::Error { source: None })"
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
    // Test the various response cases for the 'register_enclave' & 'deregister_enclave' endpoint
    async fn register_deregister_enclave_test() {
        let app = test::init_service(new_app(generate_app_state().await)).await;

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

        // Deregister the enclave without even injecting the private key
        let req = test::TestRequest::delete().uri("/deregister").to_request();

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

        // Deregister the enclave before even registering it
        let req = test::TestRequest::delete().uri("/deregister").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Enclave not registered yet!"
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

        sleep(Duration::from_secs(2)).await;
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

    // Execute a job request from event logs and return the response
    async fn job_handler_unit_test(
        job_id: U256,
        code_hash: String,
        code_inputs: Bytes,
        user_deadline: u64,
    ) -> Option<JobResponse> {
        // Initialize executor key for test enclave state
        let executor = H160::random();

        let app_state = generate_app_state().await;
        *app_state.executor_operator_key.lock().unwrap() = Some(executor);
        *app_state.registered.lock().unwrap() = true;

        let (tx, mut rx) = channel::<JobResponse>(10);

        // Fill logs for JobRelayed and JobResponded events where the latter will be streamed after some delay
        let job_logs = vec![
            Log {
                address: H160::from_str(JOBS_CONTRACT_ADDR).unwrap(),
                topics: vec![
                    keccak256(
                        "JobRelayed(uint256,bytes32,bytes,uint256,address,address,address[])",
                    )
                    .into(),
                    H256::from_uint(&job_id),
                ],
                data: encode(&[
                    Token::FixedBytes(hex::decode(code_hash).unwrap()),
                    Token::Bytes(code_inputs.into()),
                    Token::Uint(user_deadline.into()),
                    Token::Address(H160::random()),
                    Token::Address(H160::random()),
                    Token::Array(vec![Token::Address(executor)]),
                ])
                .into(),
                ..Default::default()
            },
            Log {
                address: H160::from_str(JOBS_CONTRACT_ADDR).unwrap(),
                topics: vec![
                    keccak256("JobResponded(uint256,bytes,uint256,uint8,uint8)").into(),
                    H256::from_uint(&job_id),
                ],
                data: encode(&[
                    Token::Bytes([].into()),
                    Token::Uint(U256::one()),
                    Token::Uint((0 as u8).into()),
                    Token::Uint((1 as u8).into()),
                ])
                .into(),
                ..Default::default()
            },
        ];

        tokio::spawn(async move {
            let jobs_stream = std::pin::pin!(tokio_stream::iter(job_logs.into_iter()).then(
                |log| async move {
                    sleep(Duration::from_millis(user_deadline)).await;
                    log
                }
            ));

            // Call the event handler for the contract logs
            handle_event_logs(
                jobs_stream,
                std::pin::pin!(tokio_stream::pending()),
                app_state,
                tx,
            )
            .await;
        });

        // Receive and return the response
        while let Some(job_response) = rx.recv().await {
            return Some(job_response);
        }

        return None;
    }

    #[actix_web::test]
    // Test a valid job request with different inputs and verify the response
    async fn valid_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,5");

        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 20
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,2,5");

        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 600
        }))
        .unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(execution_response.output, "2,2,2,3,5,5");
    }

    #[actix_web::test]
    // Test a valid job request with invalid input and verify the response
    async fn invalid_input_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({})).unwrap();

        let job_response = job_handler_unit_test(
            1.into(),
            "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 0);
        assert_eq!(
            execution_response.output,
            "Please provide a valid integer as input in the format{'num':10}"
        );
    }

    #[actix_web::test]
    // Test '1' error code job requests and verify the response
    async fn invalid_transaction_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // Given transaction hash doesn't belong to the expected smart contract
        let job_response = job_handler_unit_test(
            1.into(),
            "fed8ab36cc27831836f6dcb7291049158b4d8df31c0ffb05a3d36ba6555e29d7".to_owned(),
            code_input_bytes.clone().into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 1);
        assert_eq!(execution_response.output, "");

        // Given transaction hash doesn't exist in the expected rpc network
        let job_response = job_handler_unit_test(
            1.into(),
            "37b0b2d9dd58d9130781fc914da456c16ec403010e8d4c27b0ea4657a24c8546".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 1);
        assert_eq!(execution_response.output, "");
    }

    #[actix_web::test]
    // Test '3' error code job request and verify the response
    async fn invalid_code_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // Code corresponding to the provided transaction hash has a syntax error
        let job_response = job_handler_unit_test(
            1.into(),
            "96179f60fd7917c04ad9da6dd64690a1a960f39b50029d07919bf2628f5e7fe5".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 3);
        assert_eq!(execution_response.output, "");
    }

    #[actix_web::test]
    // Test '4' error code job request and verify the response
    async fn deadline_timeout_job_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        // User code didn't return a response in the expected period
        let job_response = job_handler_unit_test(
            1.into(),
            "9c641b535e5586200d0f2fd81f05a39436c0d9dd35530e9fb3ca18352c3ba111".to_owned(),
            code_input_bytes.into(),
            2000,
        )
        .await;

        assert!(job_response.is_some());
        let job_response = job_response.unwrap();
        assert!(job_response.execution_response.is_some());
        assert!(job_response.timeout_response.is_none());
        let execution_response = job_response.execution_response.unwrap();
        assert_eq!(execution_response.id, 1.into());
        assert_eq!(execution_response.error_code, 4);
        assert_eq!(execution_response.output, "");
    }

    #[actix_web::test]
    // Test the execution timeout case where enough job responses are not received and slashing transaction should be sent for the job request
    async fn timeout_job_execution_test() {
        let code_input_bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap();

        let app_state = generate_app_state().await;
        *app_state.executor_operator_key.lock().unwrap() = Some(H160::random());
        *app_state.registered.lock().unwrap() = true;

        let (tx, mut rx) = channel::<JobResponse>(10);

        // Add log entry to relay a job but job response event is not sent and the executor doesn't execute the job request
        let job_logs = vec![Log {
            address: H160::from_str(JOBS_CONTRACT_ADDR).unwrap(),
            topics: vec![
                keccak256("JobRelayed(uint256,bytes32,bytes,uint256,address,address,address[])")
                    .into(),
                H256::from_uint(&1.into()),
            ],
            data: encode(&[
                Token::FixedBytes(
                    hex::decode("96179f60fd7917c04ad9da6dd64690a1a960f39b50029d07919bf2628f5e7fe5")
                        .unwrap(),
                ),
                Token::Bytes(code_input_bytes.into()),
                Token::Uint(2000.into()),
                Token::Address(H160::random()),
                Token::Address(H160::random()),
                Token::Array(vec![Token::Address(H160::random())]),
            ])
            .into(),
            ..Default::default()
        }];

        tokio::spawn(async move {
            let jobs_stream = std::pin::pin!(tokio_stream::iter(job_logs.into_iter()));

            handle_event_logs(
                jobs_stream,
                std::pin::pin!(tokio_stream::pending()),
                app_state,
                tx,
            )
            .await;
        });

        while let Some(job_response) = rx.recv().await {
            assert!(job_response.execution_response.is_none());
            assert!(job_response.timeout_response.is_some());
            assert_eq!(job_response.timeout_response.unwrap(), 1.into());
            return;
        }

        panic!("TEST FAILED: Timeout response not found!!!");
    }

    #[actix_web::test]
    // Test ExecutorDeregistered event handling
    async fn executor_deregistered_test() {
        let executor = H160::random();

        let app_state = generate_app_state().await;
        *app_state.executor_operator_key.lock().unwrap() = Some(executor);
        *app_state.registered.lock().unwrap() = true;

        let (tx, mut rx) = channel::<JobResponse>(10);

        // Add log for deregistering the current executor
        let executor_logs = vec![Log {
            address: H160::from_str(EXECUTORS_CONTRACT_ADDR).unwrap(),
            topics: vec![
                keccak256("ExecutorDeregistered(address)").into(),
                H256::from(executor),
            ],
            ..Default::default()
        }];

        tokio::spawn(async move {
            let executors_stream = std::pin::pin!(
                tokio_stream::iter(executor_logs.into_iter()).chain(tokio_stream::pending())
            );

            handle_event_logs(
                std::pin::pin!(tokio_stream::pending()),
                executors_stream,
                app_state,
                tx,
            )
            .await;
        });

        loop {
            let job_response = rx.recv().await;
            assert!(job_response.is_none());
            return;
        }
    }
}
