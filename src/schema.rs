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
    sync (key) {
        #[max_length = 16]
        key -> Varchar,
        value -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    providers,
    sync,
);
