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
        #[max_length = 66]
        rate -> Bpchar,
        #[max_length = 66]
        balance -> Bpchar,
        last_settled -> Timestamp,
        created -> Timestamp,
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
    sync (block) {
        block -> Int8,
    }
}

diesel::joinable!(jobs -> providers (provider));

diesel::allow_tables_to_appear_in_same_query!(
    jobs,
    providers,
    sync,
);
