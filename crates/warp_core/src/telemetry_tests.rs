use super::EnablementState;
use crate::channel::Channel;
use crate::features::FeatureFlag;

#[test]
fn telemetry_is_disabled_for_the_zyh_product() {
    assert!(!EnablementState::Always.is_enabled());
    assert!(!EnablementState::Flag(FeatureFlag::AgentMode).is_enabled());
    assert!(!EnablementState::ChannelSpecific {
        channels: vec![Channel::Stable],
    }
    .is_enabled());
}
