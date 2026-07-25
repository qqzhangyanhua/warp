use serde::{Deserialize, Serialize};
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::{Entity, ModelContext};

// Note: The name of this key is from before this model was created. For consistency, it should
// remain the same value
const SENT_REFERRAL_THEME_KEY: &str = "ReferralThemeActive";
const RECEIVED_REFERRAL_THEME_KEY: &str = "ReceivedReferralTheme";

/// Events historically emitted when a referral theme became available via network.
///
/// ZYH no longer fetches referral status from the server. Themes can only be
/// unlocked from already-persisted local preferences; these events are retained
/// so theme-chooser subscriptions keep compiling but are never emitted.
#[allow(dead_code)]
pub enum ReferralThemeEvent {
    SentReferralThemeActivated,
    ReceivedReferralThemeActivated,
}

/// Local-only model tracking whether referral reward themes are unlocked.
///
/// Themes are never revoked once active. ZYH does not query Warp referral
/// GraphQL; unlock state comes only from private user preferences.
pub struct ReferralThemeStatus {
    sent_referral_theme: ReferralThemeFetchStatus,
    received_referral_theme: ReferralThemeFetchStatus,
}

impl Entity for ReferralThemeStatus {
    type Event = ReferralThemeEvent;
}

impl ReferralThemeStatus {
    /// Creates a new ReferralThemeStatus model from local user defaults only.
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let sent_referral_theme = parse_sent_referral_fetch_status(
            ctx.private_user_preferences()
                .read_value(SENT_REFERRAL_THEME_KEY)
                .unwrap_or_default(),
        );

        let received_referral_theme = ctx
            .private_user_preferences()
            .read_value(RECEIVED_REFERRAL_THEME_KEY)
            .unwrap_or_default()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(ReferralThemeFetchStatus::NotFetched);

        Self {
            sent_referral_theme,
            received_referral_theme,
        }
    }

    /// Is the "Sent Referral" theme available (i.e. has the user sent at least one referral)?
    pub fn sent_referral_theme_active(&self) -> bool {
        self.sent_referral_theme.is_active()
    }

    /// Is the "Received Referral" theme available (i.e. was the user referred by another)?
    pub fn received_referral_theme_active(&self) -> bool {
        self.received_referral_theme.is_active()
    }
}

/// Type used for tracking the unlock status of referral themes in local prefs.
#[derive(Serialize, Deserialize, Clone, Copy)]
enum ReferralThemeFetchStatus {
    NotFetched,
    Inactive,
    Active,
}

impl ReferralThemeFetchStatus {
    fn is_active(self) -> bool {
        matches!(self, ReferralThemeFetchStatus::Active)
    }
}

/// Parse the sent referral status into a ReferralThemeFetchStatus
///
/// Note: For historical reasons, the status is stored as a boolean literal (`true` or `false`),
/// so we need to map that onto the fetch status.
fn parse_sent_referral_fetch_status(stored_value: Option<String>) -> ReferralThemeFetchStatus {
    stored_value
        .and_then(|s| s.parse::<bool>().ok())
        .map(|active| {
            if active {
                ReferralThemeFetchStatus::Active
            } else {
                ReferralThemeFetchStatus::Inactive
            }
        })
        .unwrap_or(ReferralThemeFetchStatus::NotFetched)
}
