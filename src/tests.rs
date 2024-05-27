// TODO: tests have to be run one by one currently
/* NOTE: To run an unit test 'test_name', hit the following commands on terminal ->
   1.    sudo ./cgroupv2_setup.sh
   2.    sudo echo && cargo test 'test name' -- --nocapture &
   3.    sudo echo && cargo test -- --test-threads 1 &           (For running all the tests sequentially)
*/

#[cfg(test)]
pub mod serverless_executor_test {
    use std::collections::HashSet;
    use std::str::FromStr;

    use actix_web::body::MessageBody;
    use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
    use actix_web::web::{Bytes, Data};
    use actix_web::{http, test, App, Error};
    use ethers::abi::{encode, Token};
    use ethers::providers::{Provider, Ws};
    use ethers::types::{Address, BigEndianHash, Log, H160, H256, U256};
    use ethers::utils::{keccak256, public_key_to_address};
    use k256::ecdsa::SigningKey;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use tokio::sync::mpsc::channel;
    use tokio::time::{sleep, Duration};
    use tokio_stream::StreamExt as _;

    use crate::cgroups::Cgroups;
    use crate::event_handler::handle_event_logs;
    use crate::node_handler::{export, index, inject};
    use crate::utils::{AppState, JobResponse};

    // Testnet or Local blockchain (Hardhat) configurations
    const CHAIN_ID: u64 = 421614;
    const HTTP_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
    const WS_URL: &str = "wss://arbitrum-sepolia.infura.io/ws/v3/cd72f20b9fd544f8a5b8da706441e01c";
    const EXECUTORS_CONTRACT_ADDR: &str = "0xdec0719F26f3771D9E84Cf8694DAE79F3f2AbEbB";
    const JOBS_CONTRACT_ADDR: &str = "0xAc6Ae536203a3ec290ED4aA1d3137e6459f4A963";
    const CODE_CONTRACT_ADDR: &str = "0x44fe06d2940b8782a0a9a9ffd09c65852c0156b1";
    const ENCLAVE_KEY: &str = "2526d18e11b6bcb52b1bf9e1c2eca2b0122cfd2be6465c22670d06d4c9a1b030";
    const GAS_WALLET_KEY: &str =
        "0xa8b743563462eb4b943e3de02ce7fcdfde6ca255b2f6850f34d47c1a9824b2f8";
    const OWNER_ADDRESS: &str = "0xf90e66d1452be040ca3a82387bf6ad0c472f29dd";
    const REGISTER_TIMESTAMP: usize = 1722134849000;

    // const REGISTER_ATTESTATION: &str = "0xcfa7554f87ba13620037695d62a381a2d876b74c2e1b435584fe5c02c53393ac1c5cd5a8b6f92e866f9a65af751e0462cfa7554f87ba13620037695d62a381a2d8";
    // const REGISTER_PCR_0: &str = "0xcfa7554f87ba13620037695d62a381a2d876b74c2e1b435584fe5c02c53393ac1c5cd5a8b6f92e866f9a65af751e0462";
    // const REGISTER_PCR_1: &str = "0xbcdf05fefccaa8e55bf2c8d6dee9e79bbff31e34bf28a99aa19e6b29c37ee80b214a414b7607236edf26fcb78654e63f";
    // const REGISTER_PCR_2: &str = "0x20caae8a6a69d9b1aecdf01a0b9c5f3eafd1f06cb51892bf47cef476935bfe77b5b75714b68a69146d650683a217c5b3";
    // const REGISTER_STAKE_AMOUNT: usize = 100;

    // Generate test app state
    async fn generate_app_state() -> Data<AppState> {
        let signer = SigningKey::from_slice(&hex::decode(ENCLAVE_KEY).unwrap()).unwrap();
        let signer_verifier_address = public_key_to_address(signer.verifying_key());

        Data::new(AppState {
            job_capacity: 20,
            cgroups: Cgroups::new().unwrap().into(),
            registered: false.into(),
            register_listener_active: false.into(),
            num_selected_executors: 1,
            common_chain_id: CHAIN_ID,
            http_rpc_url: HTTP_RPC_URL.to_owned(),
            http_rpc_client: None.into(),
            web_socket_client: Provider::<Ws>::connect(WS_URL).await.unwrap(),
            executors_contract_addr: EXECUTORS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            jobs_contract_addr: JOBS_CONTRACT_ADDR.parse::<Address>().unwrap(),
            code_contract_addr: CODE_CONTRACT_ADDR.to_owned(),
            enclave_owner: None.into(),
            enclave_signer: signer,
            enclave_address: signer_verifier_address,
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
            .service(inject)
            .service(export)
    }

    #[actix_web::test]
    // Test the various response cases for the 'inject' endpoint
    async fn inject_test() {
        let app = test::init_service(new_app(generate_app_state().await)).await;

        // Inject invalid owner address hex string (less than 20 bytes)
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": "0x322557",
                "gas_key": "0x32255"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Invalid owner address provided: Invalid input length"
        );

        // Inject invalid gas private key hex string (invalid hex encoding)
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": OWNER_ADDRESS,
                "gas_key": "0x32255"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Failed to hex decode the gas key into 32 bytes: OddLength"
        );

        // Inject invalid private(signing) gas key
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": OWNER_ADDRESS,
                "gas_key": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Invalid gas key provided: EcdsaError(signature::Error { source: None })"
        );

        // Inject a valid gas key and owner address
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": OWNER_ADDRESS,
                "gas_key": GAS_WALLET_KEY,
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Injection done successfully"
        );

        // Inject the valid keys again
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": OWNER_ADDRESS,
                "gas_key": GAS_WALLET_KEY,
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Injection already done!"
        );
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ExportResponse {
        job_capacity: usize,
        sign_timestamp: usize,
        owner: H160,
        signature: String,
    }

    #[actix_web::test]
    // Test the various response cases for the 'export' endpoint
    async fn export_test() {
        let app = test::init_service(new_app(generate_app_state().await)).await;

        // Export the enclave registration details without injecting gas key and owner address
        let req = test::TestRequest::post()
            .uri("/export")
            .set_json(&json!({
                "timestamp": REGISTER_TIMESTAMP
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Injection not done yet!"
        );

        // Inject a valid private gas key and owner address
        let req = test::TestRequest::post()
            .uri("/inject")
            .set_json(&json!({
                "owner_address": OWNER_ADDRESS,
                "gas_key": GAS_WALLET_KEY,
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.into_body().try_into_bytes().unwrap(),
            "Injection done successfully"
        );

        // Export the enclave registration details
        let req = test::TestRequest::post()
            .uri("/export")
            .set_json(&json!({
                "timestamp": REGISTER_TIMESTAMP,
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        let response: Result<ExportResponse, serde_json::Error> =
            serde_json::from_slice(&resp.into_body().try_into_bytes().unwrap());
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.job_capacity, 20);
        assert_eq!(response.sign_timestamp, REGISTER_TIMESTAMP);
        assert_eq!(hex::encode(response.owner.0), OWNER_ADDRESS[2..]);
        assert_eq!(response.signature, "20a9541b2a245c7c87ba8ede0d10947382021c2bf034358370a2eb700f7efb2327c8d5f1fc2cff3695e0270ff9795394ba180a40a21937d4e2b2aef928dd63651c");
    }

    #[actix_web::test]
    // Test a valid job request with different inputs and verify the responses
    async fn valid_job_test() {
        let app_state = generate_app_state().await;

        let code_hash = "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e";
        let user_deadline = 5000;

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap()
        .into();

        // Prepare the logs for JobCreated and JobResponded events accordingly
        let mut job_logs = vec![
            get_job_created_log(
                0.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(0.into()),
        ];

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({
            "num": 20
        }))
        .unwrap()
        .into();

        job_logs.append(&mut vec![
            get_job_created_log(
                1.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(1.into()),
        ]);

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({
            "num": 600
        }))
        .unwrap()
        .into();

        job_logs.append(&mut vec![
            get_job_created_log(
                2.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(2.into()),
        ]);

        job_logs.push(Log {
            ..Default::default()
        });

        let (tx, mut rx) = channel::<JobResponse>(10);

        tokio::spawn(async move {
            // Introduce time interval between events to be polled
            let jobs_stream = std::pin::pin!(tokio_stream::iter(job_logs.into_iter()).then(
                |log| async move {
                    sleep(Duration::from_millis(user_deadline)).await;
                    log
                }
            ));

            // Call the event handler for the contract logs
            handle_event_logs(
                jobs_stream,
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 3);

        assert_response(responses[0].clone(), 0.into(), 0, "2,5");
        assert_response(responses[1].clone(), 1.into(), 0, "2,2,5");
        assert_response(responses[2].clone(), 2.into(), 0, "2,2,2,3,5,5");
    }

    #[actix_web::test]
    // Test a valid job request with invalid input and verify the response
    async fn invalid_input_job_test() {
        let app_state = generate_app_state().await;

        let code_hash = "9468bb6a8e85ed11e292c8cac0c1539df691c8d8ec62e7dbfa9f1bd7f504e46e";
        let user_deadline = 5000;

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({})).unwrap().into();

        let job_logs = vec![
            get_job_created_log(
                0.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(0.into()),
            Log {
                ..Default::default()
            },
        ];

        let (tx, mut rx) = channel::<JobResponse>(10);

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
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 1);

        assert_response(
            responses[0].clone(),
            0.into(),
            0,
            "Please provide a valid integer as input in the format{'num':10}",
        );
    }

    #[actix_web::test]
    // Test '1' error code job requests and verify the responses
    async fn invalid_transaction_job_test() {
        let app_state = generate_app_state().await;

        let user_deadline = 5000;
        let code_input_bytes: Bytes = serde_json::to_vec(&json!({
            "num": 10
        }))
        .unwrap()
        .into();

        // Given transaction hash doesn't belong to the expected smart contract
        let mut job_logs = vec![
            get_job_created_log(
                0.into(),
                "fed8ab36cc27831836f6dcb7291049158b4d8df31c0ffb05a3d36ba6555e29d7",
                code_input_bytes.clone(),
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(0.into()),
        ];

        // Given transaction hash doesn't exist in the expected rpc network
        job_logs.append(&mut vec![
            get_job_created_log(
                1.into(),
                "37b0b2d9dd58d9130781fc914da456c16ec403010e8d4c27b0ea4657a24c8546",
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(1.into()),
        ]);

        job_logs.push(Log {
            ..Default::default()
        });

        let (tx, mut rx) = channel::<JobResponse>(10);

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
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 2);

        assert_response(responses[0].clone(), 0.into(), 1, "");
        assert_response(responses[1].clone(), 1.into(), 1, "");
    }

    #[actix_web::test]
    // Test '2' error code job request and verify the response
    async fn invalid_code_job_test() {
        let app_state = generate_app_state().await;

        let code_hash = "96179f60fd7917c04ad9da6dd64690a1a960f39b50029d07919bf2628f5e7fe5";
        let user_deadline = 5000;

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({})).unwrap().into();

        // Code corresponding to the provided transaction hash has a syntax error
        let job_logs = vec![
            get_job_created_log(
                0.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(0.into()),
            Log {
                ..Default::default()
            },
        ];

        let (tx, mut rx) = channel::<JobResponse>(10);

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
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 1);

        assert_response(responses[0].clone(), 0.into(), 2, "");
    }

    #[actix_web::test]
    // Test '3' error code job request and verify the response
    async fn deadline_timeout_job_test() {
        let app_state = generate_app_state().await;

        let code_hash = "9c641b535e5586200d0f2fd81f05a39436c0d9dd35530e9fb3ca18352c3ba111";
        let user_deadline = 5000;

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({})).unwrap().into();

        // User code didn't return a response in the expected period
        let job_logs = vec![
            get_job_created_log(
                0.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                app_state.enclave_address,
            ),
            get_job_responded_log(0.into()),
            Log {
                ..Default::default()
            },
        ];

        let (tx, mut rx) = channel::<JobResponse>(10);

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
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 1);

        assert_response(responses[0].clone(), 0.into(), 3, "");
    }

    #[actix_web::test]
    // Test the execution timeout case where enough job responses are not received and slashing transaction should be sent for the job request
    async fn timeout_job_execution_test() {
        let app_state = generate_app_state().await;

        let code_hash = "9c641b535e5586200d0f2fd81f05a39436c0d9dd35530e9fb3ca18352c3ba111";
        let user_deadline = 5000;
        let execution_buffer_time = app_state.execution_buffer_time;

        let code_input_bytes: Bytes = serde_json::to_vec(&json!({})).unwrap().into();

        // Add log entry to relay a job but job response event is not sent and the executor doesn't execute the job request
        let job_logs = vec![
            get_job_created_log(
                0.into(),
                code_hash,
                code_input_bytes,
                user_deadline,
                H160::random(),
            ),
            Log {
                ..Default::default()
            },
        ];

        let (tx, mut rx) = channel::<JobResponse>(10);

        tokio::spawn(async move {
            let jobs_stream = std::pin::pin!(tokio_stream::iter(job_logs.into_iter()).then(
                |log| async move {
                    sleep(Duration::from_millis(
                        user_deadline + execution_buffer_time * 1000 + 1000,
                    ))
                    .await;
                    log
                }
            ));

            // Call the event handler for the contract logs
            handle_event_logs(
                jobs_stream,
                std::pin::pin!(tokio_stream::empty()),
                app_state,
                tx,
            )
            .await;
        });

        let mut responses: Vec<JobResponse> = vec![];

        // Receive and store the responses
        while let Some(job_response) = rx.recv().await {
            responses.push(job_response);
        }

        assert_eq!(responses.len(), 1);
        let job_response = responses[0].clone();
        assert!(job_response.job_output.is_none());
        assert!(job_response.timeout_response.is_some());
        assert_eq!(job_response.timeout_response.unwrap(), 0.into());
    }

    #[actix_web::test]
    // Test ExecutorDeregistered event handling
    async fn executor_deregistered_test() {
        let app_state = generate_app_state().await;

        let (tx, mut rx) = channel::<JobResponse>(10);

        // Add log for deregistering the current executor
        let executor_logs = vec![Log {
            address: H160::from_str(EXECUTORS_CONTRACT_ADDR).unwrap(),
            topics: vec![
                keccak256("ExecutorDeregistered(address)").into(),
                H256::from(app_state.enclave_address),
            ],
            removed: Some(false),
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

        while rx.recv().await.is_some() {
            assert!(1 == 2);
        }

        assert!(1 == 1);
    }

    fn get_job_created_log(
        job_id: U256,
        code_hash: &str,
        code_inputs: Bytes,
        user_deadline: u64,
        enclave: H160,
    ) -> Log {
        Log {
            address: H160::from_str(JOBS_CONTRACT_ADDR).unwrap(),
            topics: vec![
                keccak256("JobCreated(uint256,address,bytes32,bytes,uint256,address[])").into(),
                H256::from_uint(&job_id),
                H256::from(H160::random()),
            ],
            data: encode(&[
                Token::FixedBytes(hex::decode(code_hash).unwrap()),
                Token::Bytes(code_inputs.into()),
                Token::Uint(user_deadline.into()),
                Token::Array(vec![Token::Address(enclave)]),
            ])
            .into(),
            removed: Some(false),
            ..Default::default()
        }
    }

    fn get_job_responded_log(job_id: U256) -> Log {
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
            removed: Some(false),
            ..Default::default()
        }
    }

    fn assert_response(job_response: JobResponse, id: U256, error: u8, output: &str) {
        assert!(job_response.job_output.is_some());
        assert!(job_response.timeout_response.is_none());
        let job_output = job_response.job_output.unwrap();

        assert_eq!(job_output.id, id);
        assert_eq!(job_output.execution_response.error_code, error);
        assert_eq!(job_output.execution_response.output, output);
    }
}
