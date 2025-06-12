use anyhow::Result;
use ipnetwork::IpNetwork;
use iter_tools::Itertools;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::NetworkManagement::WindowsFirewall::*;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::core::HRESULT;

const RULE_NAME: &str = "ow2dropshiprs";

pub async fn start(blocks: Vec<IpNetwork>, game_path: String) -> Result<()> {
    unsafe {
        let com = ComDrop::init();
        if com.0 != RPC_E_CHANGED_MODE {
            com.0.ok()?;
        }

        let fwpol: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;

        let rules = fwpol.Rules()?;
        rules.Remove(&RULE_NAME.into())?;

        let rule: INetFwRule = CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)?;
        rule.SetName(&RULE_NAME.into())?;
        rule.SetDescription(&"".into())?;
        rule.SetApplicationName(&game_path.into())?;
        rule.SetProtocol(NET_FW_IP_PROTOCOL_ANY.0)?;
        rule.SetRemoteAddresses(
            &blocks
                .iter()
                .map(|net| format!("{}/{}", net.ip(), net.prefix()))
                .join(",")
                .into(),
        )?;
        rule.SetEnabled(true.into())?;
        rule.SetDirection(NET_FW_RULE_DIR_OUT)?;
        rule.SetAction(NET_FW_ACTION_BLOCK)?;

        rules.Add(&rule)?;
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    unsafe {
        let com = ComDrop::init();
        if com.0 != RPC_E_CHANGED_MODE {
            com.0.ok()?;
        }

        let fwpol: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;
        let rules = fwpol.Rules()?;

        rules.Remove(&RULE_NAME.into())?;
    }

    Ok(())
}

struct ComDrop(HRESULT);
impl ComDrop {
    fn init() -> ComDrop {
        unsafe { ComDrop(CoInitializeEx(None, COINIT_APARTMENTTHREADED)) }
    }
}
impl Drop for ComDrop {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
