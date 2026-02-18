// @generated — hand-written to match document/database-schema.md
// All PostgreSQL ENUM columns are stored as Varchar.

diesel::table! {
    users (id) {
        id -> Uuid,
        line_user_id -> Varchar,
        display_name -> Varchar,
        picture_url -> Nullable<Text>,
        language -> Varchar,
        terms_accepted_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    characters (id) {
        id -> Uuid,
        name -> Varchar,
        personality -> Text,
        speaking_style -> Text,
        background -> Text,
        system_prompt -> Text,
        avatar_url -> Nullable<Text>,
        genre_tags -> Array<Text>,
        gender -> Varchar,
        is_active -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    scenes (id) {
        id -> Uuid,
        character_id -> Uuid,
        name -> Varchar,
        location -> Varchar,
        time_of_day -> Varchar,
        atmosphere -> Text,
        situation_prompt -> Text,
        opening_narrator -> Text,
        opening_dialogue -> Text,
        is_default -> Bool,
        is_active -> Bool,
        is_adult_content -> Bool,
        start_relationship_level -> Varchar,
        start_mood -> Varchar,
        image_url -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    roleplay_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        character_id -> Uuid,
        scene_id -> Uuid,
        status -> Varchar,
        mood -> Varchar,
        relationship_level -> Varchar,
        message_count -> Int4,
        current_location -> Nullable<Varchar>,
        scene_time -> Nullable<Varchar>,
        scene_summary -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    messages (id) {
        id -> Uuid,
        session_id -> Uuid,
        role -> Varchar,
        message_type -> Varchar,
        content -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    jobs (id) {
        id -> Uuid,
        session_id -> Nullable<Uuid>,
        user_id -> Uuid,
        line_user_id -> Varchar,
        user_message -> Text,
        mode -> Varchar,
        status -> Varchar,
        attempts -> Int4,
        max_attempts -> Int4,
        locked_at -> Nullable<Timestamptz>,
        completed_at -> Nullable<Timestamptz>,
        failed_reason -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    credit_balances (id) {
        id -> Uuid,
        user_id -> Uuid,
        balance -> Int4,
        total_purchased -> Int4,
        total_consumed -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    credit_transactions (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[sql_name = "type"]
        type_ -> Varchar,
        amount -> Int4,
        balance_after -> Int4,
        reference_id -> Nullable<Uuid>,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    app_config (key) {
        key -> Varchar,
        value -> Varchar,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(scenes -> characters (character_id));
diesel::joinable!(roleplay_sessions -> users (user_id));
diesel::joinable!(roleplay_sessions -> characters (character_id));
diesel::joinable!(roleplay_sessions -> scenes (scene_id));
diesel::joinable!(messages -> roleplay_sessions (session_id));
diesel::joinable!(jobs -> users (user_id));
diesel::joinable!(credit_balances -> users (user_id));
diesel::joinable!(credit_transactions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    app_config,
    users,
    characters,
    scenes,
    roleplay_sessions,
    messages,
    jobs,
    credit_balances,
    credit_transactions,
);
