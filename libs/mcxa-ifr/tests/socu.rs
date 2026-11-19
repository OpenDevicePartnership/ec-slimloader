use std::collections::BTreeSet;

use mcxa_ifr::{
    CMPA, CcSocuDflt, CcSocuPin, DebugControl, DebugKeyFlush, DebugState, DebugUuidCheck, DevcfgErase, IFR,
};

fn pin_config(choices: u16) -> CcSocuPin {
    let control = |index: u32| {
        if choices & (1u16 << index) == 0 {
            DebugControl::DebugAuthentication
        } else {
            DebugControl::Fixed
        }
    };
    CcSocuPin {
        niden: control(0),
        dbgen: control(1),
        spniden: control(2),
        spiden: control(3),
        isp_cmd: control(4),
        fa_cmd: control(5),
        me_cmd: control(6),
    }
}

fn dflt_config(choices: u16) -> CcSocuDflt {
    let state = |index: u32| {
        if choices & (1u16 << index) == 0 {
            DebugState::Disabled
        } else {
            DebugState::Enabled
        }
    };
    CcSocuDflt {
        niden: state(0),
        dbgen: state(1),
        spniden: state(2),
        spiden: state(3),
        isp_cmd: state(4),
        fa_cmd: state(5),
        me_cmd: state(6),
        devcfg_erase: if choices & 0x80 == 0 {
            DevcfgErase::Disabled
        } else {
            DevcfgErase::Enabled
        },
        key_flush: if choices & 0x100 == 0 {
            DebugKeyFlush::Retain
        } else {
            DebugKeyFlush::Clear
        },
        uuid_check: if choices & 0x200 == 0 {
            DebugUuidCheck::SharedCertificate
        } else {
            DebugUuidCheck::PerDeviceCertificate
        },
    }
}

#[test]
fn every_pin_configuration_has_the_documented_encoding() {
    let mut encodings = BTreeSet::new();
    let mut cmpa = CMPA::ZERO;
    cmpa.boot_cfg0.set_marker(0x5963);
    cmpa.set_cc_socu_dflt(dflt_config(0x300));
    let baseline = *cmpa.as_array();
    for choices in 0..128 {
        let config = pin_config(choices);
        let word = config.encode();
        let direct = word as u16;
        // IFR.xlsx DEVCFG row 151: bits 4:5 and 9:12 are zero;
        // bits 13:15 are one. The seven independent controls occupy these bits.
        assert_eq!(direct & 0xFE30, 0xE000);
        for (index, bit) in [0, 1, 2, 3, 6, 7, 8].into_iter().enumerate() {
            assert_eq!(direct & (1 << bit) != 0, choices & (1 << index) != 0);
        }
        assert_eq!((word >> 16) as u16, !direct);
        assert_ne!(word, 0x1FFF_0000); // PR #65: inconsistent inverse/reserved bits.
        assert!(encodings.insert(word), "PIN configurations must not collide");

        cmpa.set_cc_socu_pin(config);
        let mut expected = baseline;
        expected[0x40..0x44].copy_from_slice(&word.to_le_bytes());
        assert_eq!(cmpa.as_array(), &expected);
        assert_eq!(cmpa.cc_socu_pin(), word);
    }
    assert_eq!(encodings.len(), 128);
}

#[test]
fn every_dflt_configuration_has_the_documented_encoding() {
    let mut encodings = BTreeSet::new();
    let mut cmpa = CMPA::ZERO;
    cmpa.boot_cfg0.set_marker(0x5963);
    cmpa.set_cc_socu_pin(pin_config(0));
    let baseline = *cmpa.as_array();
    for choices in 0..1024 {
        let config = dflt_config(choices);
        let word = config.encode();
        let direct = word as u16;
        // IFR.xlsx DEVCFG row 152: reserved bits 4:5 and 9:12 are zero.
        assert_eq!(direct & 0x1E30, 0);
        for (index, bit) in [0, 1, 2, 3, 6, 7, 8, 13, 14, 15].into_iter().enumerate() {
            assert_eq!(direct & (1 << bit) != 0, choices & (1 << index) != 0);
        }
        assert_eq!((word >> 16) as u16, !direct);
        assert!(encodings.insert(word), "DFLT configurations must not collide");

        cmpa.set_cc_socu_dflt(config);
        let mut expected = baseline;
        expected[0x44..0x48].copy_from_slice(&word.to_le_bytes());
        assert_eq!(cmpa.as_array(), &expected);
        assert_eq!(cmpa.cc_socu_dflt(), word);
    }
    assert_eq!(encodings.len(), 1024);
}

#[test]
fn provisioning_choices_encode_to_the_reference_words() {
    const PIN: u32 = CcSocuPin {
        niden: DebugControl::DebugAuthentication,
        dbgen: DebugControl::DebugAuthentication,
        spniden: DebugControl::DebugAuthentication,
        spiden: DebugControl::DebugAuthentication,
        isp_cmd: DebugControl::DebugAuthentication,
        fa_cmd: DebugControl::DebugAuthentication,
        me_cmd: DebugControl::DebugAuthentication,
    }
    .encode();
    const DFLT: u32 = CcSocuDflt {
        niden: DebugState::Disabled,
        dbgen: DebugState::Disabled,
        spniden: DebugState::Disabled,
        spiden: DebugState::Disabled,
        isp_cmd: DebugState::Disabled,
        fa_cmd: DebugState::Disabled,
        me_cmd: DebugState::Disabled,
        devcfg_erase: DevcfgErase::Disabled,
        key_flush: DebugKeyFlush::Clear,
        uuid_check: DebugUuidCheck::PerDeviceCertificate,
    }
    .encode();
    assert_eq!(PIN, 0x1FFF_E000);
    assert_eq!(DFLT, 0x3FFF_C000);
}

#[test]
fn zeroed_socu_words_remain_raw() {
    for cmpa in [CMPA::ZERO, IFR::zeroed().cmpa] {
        assert_eq!(cmpa.cc_socu_pin(), 0);
        assert_eq!(cmpa.cc_socu_dflt(), 0);
        assert_eq!(&cmpa.as_array()[0x40..0x48], &[0; 8]);
    }
}
