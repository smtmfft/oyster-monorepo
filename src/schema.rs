// @generated automatically by Diesel CLI.

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

diesel::allow_tables_to_appear_in_same_query!(
    providers,
    sync,
);
