use nl2sh::{
    config::ExecuteUserMode,
    shell::{resolve_invocation, RootProbe},
};
struct Probe {
    uid: u32,
    su: bool,
}
impl RootProbe for Probe {
    fn uid(&self) -> u32 {
        self.uid
    }
    fn su_available(&self) -> bool {
        self.su
    }
}
#[test]
fn root_matrix() {
    let root = Probe { uid: 0, su: false };
    let user = Probe {
        uid: 2000,
        su: false,
    };
    let su = Probe {
        uid: 2000,
        su: true,
    };
    assert_ne!(
        resolve_invocation("id", ExecuteUserMode::Auto, false, &root)
            .unwrap()
            .0,
        "su"
    );
    assert_ne!(
        resolve_invocation("id", ExecuteUserMode::Normal, true, &su)
            .unwrap()
            .0,
        "su"
    );
    assert_ne!(
        resolve_invocation("id", ExecuteUserMode::Auto, false, &su)
            .unwrap()
            .0,
        "su"
    );
    assert_eq!(
        resolve_invocation("id", ExecuteUserMode::Auto, true, &su)
            .unwrap()
            .0,
        "su"
    );
    assert_eq!(
        resolve_invocation(
            "echo 'a' | sed \"s/a/b/\"",
            ExecuteUserMode::Root,
            true,
            &su
        )
        .unwrap()
        .1[1],
        "echo 'a' | sed \"s/a/b/\""
    );
    assert!(resolve_invocation("id", ExecuteUserMode::Root, true, &user).is_err());
}
