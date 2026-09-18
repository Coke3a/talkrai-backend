use talkrai_backend::domain::web::Persona;
#[test]
fn persona_limits_count_characters_and_reject_blank_name() {
    assert!(Persona::new(" ".into(), "".into()).is_err());
    assert!(Persona::new("ก".repeat(80), "ข".repeat(1000)).is_ok());
    assert!(Persona::new("ก".repeat(81), "".into()).is_err());
}
