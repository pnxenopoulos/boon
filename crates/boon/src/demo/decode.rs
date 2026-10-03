use prost::Message;

/// Helper macro: expands to a `match` that tries `prost::Message::decode`
/// for each `(msg_type => ProtoType)` pair and pretty-prints the result.
macro_rules! decode_match {
    ($msg_type:expr, $data:expr, $($val:expr => $ty:ty),* $(,)?) => {
        match $msg_type {
            $($val => <$ty>::decode($data).ok().map(|m| format!("{:#?}", m)),)*
            _ => None,
        }
    }
}

/// Name an ability-change event using the client version in the demo's game path.
/// Value 4 is ambiguous when the path does not identify a version.
pub fn ability_change_name(change: Option<i32>, game_directory: Option<&str>) -> &'static str {
    use boon_proto::proto::c_citadel_user_msg_abilities_changed::Change;
    let Ok(change) = Change::try_from(change.unwrap_or(-1)) else {
        return "unknown";
    };
    match change {
        Change::EPurchased => "purchased",
        Change::EUpgraded => "upgraded",
        Change::ESold => "sold",
        Change::ESwappedActivatedAbility => "swapped",
        Change::ELeveledUp => {
            let version = game_directory.and_then(|path| {
                path.rsplit(['/', '\\'])
                    .find_map(|part| part.strip_prefix("citadel_v")?.parse::<u32>().ok())
            });
            // Upstream 58b3529c (client 6711) reassigned value 4 from failure
            // to level-up. This is a wire-format boundary, not a gameplay rule.
            match version {
                Some(version) if version < 6711 => "failure",
                Some(_) => "leveled_up",
                None => "unknown",
            }
        }
        Change::EFailure => "failure",
        Change::EInvalid => "unknown",
    }
}

/// Attempt to decode raw protobuf bytes for a known event message type.
///
/// Returns `Some(pretty-printed string)` if the type is recognized and the
/// payload decodes successfully, or `None` otherwise.
pub fn decode_event_payload(msg_type: u32, data: &[u8]) -> Option<String> {
    use boon_proto::proto::*;

    decode_match!(msg_type, data,
        // CitadelUserMessageIds (300–373)
        300 => CCitadelUserMessageDamage,
        303 => CCitadelUserMsgMapPing,
        304 => CCitadelUserMsgTeamRewards,
        308 => CCitadelUserMsgTriggerDamageFlash,
        309 => CCitadelUserMsgAbilitiesChanged,
        310 => CCitadelUserMsgRecentDamageSummary,
        311 => CCitadelUserMsgSpectatorTeamChanged,
        312 => CCitadelUserMsgChatWheel,
        313 => CCitadelUserMsgGoldHistory,
        314 => CCitadelUserMsgChatMsg,
        315 => CCitadelUserMsgQuickResponse,
        316 => CCitadelUserMsgPostMatchDetails,
        317 => CCitadelUserMsgChatEvent,
        318 => CCitadelUserMsgAbilityInterrupted,
        319 => CCitadelUserMsgHeroKilled,
        320 => CCitadelUserMsgReturnIdol,
        321 => CCitadelUserMsgSetClientCameraAngles,
        322 => CCitadelUserMsgMapLine,
        323 => CCitadelUserMessageBulletHit,
        324 => CCitadelUserMessageObjectiveMask,
        325 => CCitadelUserMessageModifierApplied,
        326 => CCitadelUserMsgCameraController,
        327 => CCitadelUserMessageAuraModifierApplied,
        329 => CCitadelUserMsgObstructedShotFired,
        330 => CCitadelUserMsgAbilityLateFailure,
        331 => CCitadelUserMsgAbilityPing,
        332 => CCitadelUserMsgPostProcessingAnim,
        333 => CCitadelUserMsgDeathReplayData,
        334 => CCitadelUserMsgPlayerLifetimeStatInfo,
        336 => CCitadelUserMsgForceShopClosed,
        337 => CCitadelUserMsgStaminaConsumed,
        338 => CCitadelUserMsgAbilityNotify,
        339 => CCitadelUserMsgGetDamageStatsResponse,
        340 => CCitadelUserMsgParticipantStartSoundEvent,
        341 => CCitadelUserMsgParticipantStopSoundEvent,
        342 => CCitadelUserMsgParticipantStopSoundEventHash,
        343 => CCitadelUserMsgParticipantSetSoundEventParams,
        344 => CCitadelUserMsgParticipantSetLibraryStackFields,
        345 => CCitadelUserMsgCurrencyChanged,
        346 => CCitadelUserMessageGameOver,
        347 => CCitadelUserMsgBossKilled,
        348 => CCitadelUserMsgBossDamaged,
        349 => CCitadelUserMsgMidBossSpawned,
        350 => CCitadelUserMsgRejuvStatus,
        351 => CCitadelUserMsgKillStreak,
        352 => CCitadelUserMsgTeamMsg,
        353 => CCitadelUserMsgPlayerRespawned,
        354 => CCitadelUserMsgCallCheaterVote,
        355 => CCitadelUserMessageMeleeHit,
        356 => CCitadelUserMsgFlexSlotUnlocked,
        357 => CCitadelUserMsgSeasonalKill,
        358 => CCitadelUserMsgMusicQueue,
        360 => CCitadelUserMessageItemPurchaseNotification,
        361 => CCitadelUserMsgEntityPortalled,
        362 => CCitadelUserMsgStreetBrawlScoring,
        363 => CCitadelUserMsgHudGameAnnouncement,
        364 => CCitadelUserMsgItemDraftReaction,
        365 => CCitadelUserMessageImportantAbilityUsed,
        366 => CCitadelUserMsgBannedHeroes,
        367 => CMsgCitadelCombatLogEntry,
        368 => CCitadelUserMsgCombatLogBulkData,
        369 => CCitadelUserMsgPlayerTyping,
        370 => CCitadelUserMsgChangeHeroStatus,
        371 => CCitadelUserMsgLocalLobby,
        372 => CCitadelUserMsgSoulBagPickup,
        373 => CCitadelUserMsgHeroReleaseVote,

        // ECitadelGameEvents (450–466)
        450 => CMsgFireBullets,
        451 => CMsgPlayerAnimEvent,
        458 => CMsgParticleSystemManager,
        459 => CMsgScreenTextPretty,
        461 => CMsgBulletImpact,
        462 => CMsgEnableSatVolumesEvent,
        463 => CMsgPlaceSatVolumeEvent,
        464 => CMsgDisableSatVolumesEvent,
        465 => CMsgRemoveSatVolumeEvent,
        466 => CMsgRemoveBullet,

        // EBaseUserMessages (101–170)
        101 => CUserMessageAchievementEvent,
        104 => CUserMessageCurrentTimescale,
        105 => CUserMessageDesiredTimescale,
        106 => CUserMessageFade,
        107 => CUserMessageGameTitle,
        110 => CUserMessageHudMsg,
        111 => CUserMessageHudText,
        113 => CUserMessageColoredText,
        114 => CUserMessageRequestState,
        115 => CUserMessageResetHud,
        116 => CUserMessageRumble,
        117 => CUserMessageSayText,
        118 => CUserMessageSayText2,
        119 => CUserMessageSayTextChannel,
        120 => CUserMessageShake,
        121 => CUserMessageShakeDir,
        122 => CUserMessageWaterShake,
        124 => CUserMessageTextMsg,
        125 => CUserMessageScreenTilt,
        128 => CUserMessageVoiceMask,
        130 => CUserMessageSendAudio,
        131 => CUserMessageItemPickup,
        132 => CUserMessageAmmoDenied,
        134 => CUserMessageShowMenu,
        135 => CUserMessageCreditsMsg,
        142 => CUserMessageCloseCaptionPlaceholder,
        143 => CUserMessageCameraTransition,
        144 => CUserMessageAudioParameter,
        145 => CUserMsgParticleManager,
        146 => CUserMsgHudError,
        148 => CUserMsgCustomGameEvent,
        149 => CUserMessageAnimStateGraphState,
        150 => CUserMessageHapticsManagerPulse,
        151 => CUserMessageHapticsManagerEffect,
        153 => CUserMessageUpdateCssClasses,
        154 => CUserMessageServerFrameTime,
        155 => CUserMessageLagCompensationError,
        156 => CUserMessageRequestDllStatus,
        157 => CUserMessageRequestUtilAction,
        158 => CUserMessageUtilMsgResponse,
        159 => CUserMessageDllStatus,
        160 => CUserMessageRequestInventory,
        161 => CUserMessageInventoryResponse,
        162 => CUserMessageRequestDiagnostic,
        163 => CUserMessageDiagnosticResponse,
        164 => CUserMessageExtraUserData,
        165 => CUserMessageNotifyResponseFound,
        166 => CUserMessagePlayResponseConditional,
        167 => CUserMessageUserSentBugBug,
        168 => CUserMessageUsageReport,
        169 => CUserMessageRemoteServerCommand,
        170 => CUserMessageRemoteServerResponse,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_change_labels_respect_the_protocol_boundary() {
        for (directory, expected) in [
            (Some("/opt/srcds/citadel_v6710/citadel"), "failure"),
            (Some("/opt/srcds/citadel_v6711/citadel"), "leveled_up"),
            (Some(r"C:\games\citadel_v6712\citadel"), "leveled_up"),
            (Some("/games/citadel"), "unknown"),
            (Some("/games/citadel_vinvalid/citadel"), "unknown"),
            (None, "unknown"),
        ] {
            assert_eq!(ability_change_name(Some(4), directory), expected);
        }
        for (value, expected) in [
            (0, "purchased"),
            (1, "upgraded"),
            (2, "sold"),
            (3, "swapped"),
            (5, "failure"),
            (-1, "unknown"),
            (100, "unknown"),
        ] {
            assert_eq!(ability_change_name(Some(value), None), expected);
        }
        assert_eq!(ability_change_name(None, None), "unknown");
    }

    #[test]
    fn new_user_messages_decode_with_their_upstream_ids() {
        use boon_proto::proto::CitadelUserMessageIds as Msg;
        for message in [
            Msg::KEUserMsgCombatLogEntry,
            Msg::KEUserMsgCombatLogBulkData,
            Msg::KEUserMsgMusicQueue,
            Msg::KEUserMsgSoulBagPickup,
            Msg::KEUserMsgHeroReleaseVote,
        ] {
            assert!(decode_event_payload(message as u32, &[]).is_some());
        }
    }

    #[test]
    fn unknown_msg_type_returns_none() {
        assert!(decode_event_payload(0, &[]).is_none());
        assert!(decode_event_payload(9999, &[1, 2, 3]).is_none());
    }

    #[test]
    fn known_type_invalid_bytes_returns_none() {
        // 300 is a known type (CCitadelUserMessageDamage), but garbage bytes
        // may or may not decode (protobuf is lenient), so we just verify no panic
        let result = decode_event_payload(300, &[0xFF, 0xFF, 0xFF, 0xFF]);
        // Result can be Some or None depending on protobuf leniency - just ensure no panic
        let _ = result;
    }

    #[test]
    fn known_type_empty_bytes_returns_some() {
        // Empty bytes should decode as an empty protobuf message
        let result = decode_event_payload(300, &[]);
        assert!(result.is_some());
    }
}
