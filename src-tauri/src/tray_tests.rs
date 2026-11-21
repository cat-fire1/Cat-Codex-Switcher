// Included by tray.rs so the production module stays small.
use super::*;

#[test]
fn numeric_icons_cover_boundaries_and_stale_state() {
    for value in [0, 9, 10, 99, 100] {
        let normal = number_icon(value, false);
        let stale = number_icon(value, true);
        assert_eq!((normal.width(), normal.height()), (32, 32));
        assert_eq!(normal.rgba().len(), 32 * 32 * 4);
        assert!(normal.rgba().chunks(4).any(|pixel| pixel[3] == 255));
        assert_ne!(normal.rgba(), stale.rgba());
    }
}

#[test]
fn percentage_uses_selected_window_and_keeps_cached_values() {
    let mut snapshot = Snapshot::default();
    assert!(percentage(&snapshot).is_none());
    let profile = crate::model::Identity {
        account_id: Some("fake".into()),
        user_id: Some("test".into()),
        email: None,
    }
    .profile();
    snapshot.active = Some(profile.id);
    snapshot.rows.push(crate::worker::Row {
        profile,
        title: "Test".into(),
        stale: true,
        fetched_at: None,
        error: None,
        subscription: None,
        usage: Some(crate::model::Usage {
            remaining_percent: 42,
            five_hour_remaining_percent: Some(15),
            resets_at: None,
            five_hour_resets_at: None,
            reset_credits_remaining: None,
            reset_credits: None,
        }),
    });
    assert_eq!(percentage(&snapshot), Some((15, true)));
    snapshot.settings.status_bar_usage_window = "weekly".into();
    assert_eq!(percentage(&snapshot), Some((42, true)));
    snapshot.settings.status_bar_usage_window = "fiveHour".into();
    snapshot.rows[0]
        .usage
        .as_mut()
        .unwrap()
        .five_hour_remaining_percent = None;
    assert_eq!(percentage(&snapshot), Some((42, true)));
    assert!(tooltip(&snapshot).contains("Cached / stale"));
}
