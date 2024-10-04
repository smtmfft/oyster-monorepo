// @generated automatically by Diesel CLI.

diesel::table! {
    jobs (id) {
        #[max_length = 66]
        id -> Bpchar,
        metadata -> Text,
        #[max_length = 42]
        owner -> Bpchar,
        #[max_length = 42]
        provider -> Bpchar,
        rate -> Numeric,
        balance -> Numeric,
        last_settled -> Timestamp,
        created -> Timestamp,
        is_closed -> Bool,
    }
}

diesel::table! {
    providers (id) {
        #[max_length = 42]
        id -> Bpchar,
        cp -> Text,
        is_active -> Bool,
    }
}

diesel::table! {
    revise_rate_requests (id) {
        #[max_length = 66]
        id -> Bpchar,
        value -> Numeric,
        updates_at -> Timestamp,
    }
}

diesel::table! {
    sync (block) {
        block -> Int8,
    }
}

diesel::joinable!(revise_rate_requests -> jobs (id));

diesel::allow_tables_to_appear_in_same_query!(
    jobs,
    providers,
    revise_rate_requests,
    sync,
);
