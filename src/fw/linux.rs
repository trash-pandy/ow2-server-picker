use std::collections::HashSet;
use std::ffi::{CStr, c_int};
use std::fs;
use std::time::Duration;

use anyhow::Result;
use ipnetwork::IpNetwork;
use iter_tools::Itertools;
use libc::{NFPROTO_IPV4, NFPROTO_IPV6};
use nftnl::expr::ToSlice;
use nftnl::*;

mod cgroup;
use cgroup::CGroup;

pub async fn start(blocks: Vec<IpNetwork>, game_path: String) -> Result<()> {
    create_tables_impl(blocks)?;

    let cgroup = CGroup::new()?;
    let mut pids = HashSet::new();
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let Ok(proc_dirs) = fs::read_dir("/proc") else {
            continue;
        };
        let new_pids: HashSet<i32> = proc_dirs
            .filter_map(|proc| {
                let proc = proc.ok()?;
                let cwd = fs::read_link(proc.path().join("cwd")).ok()?;
                if *cwd.as_os_str() == *game_path {
                    proc.file_name().to_string_lossy().parse::<i32>().ok()
                } else {
                    None
                }
            })
            .collect();
        let new_pids = new_pids.difference(&pids).cloned().collect_vec();
        for pid in new_pids {
            pids.insert(pid);
            if let Err(e) = cgroup.add(pid) {
                eprintln!("{e:#?}");
            }
        }
    }
}

fn create_tables_impl(blocks: Vec<IpNetwork>) -> Result<()> {
    stop()?;

    for chunk in blocks.chunks(50) {
        create_table(chunk)?;
    }
    Ok(())
}

fn create_table(blocks: &[IpNetwork]) -> Result<()> {
    let mut batch4 = Batch::new();
    let mut batch6 = Batch::new();
    let table4 = Table::new(&c"ow2dropshiprs", nftnl::ProtoFamily::Ipv4);
    batch4.add(&table4, MsgType::Add);

    let table6 = Table::new(&c"ow2dropshiprs", nftnl::ProtoFamily::Ipv6);
    batch6.add(&table6, MsgType::Add);

    let mut chain4 = Chain::new(&c"output", &table4);
    chain4.set_hook(Hook::Out, 500);
    chain4.set_policy(Policy::Accept);
    chain4.set_type(ChainType::Filter);
    batch4.add(&chain4, MsgType::Add);

    let mut chain6 = Chain::new(&c"output", &table6);
    chain6.set_hook(Hook::Out, 500);
    chain6.set_policy(Policy::Accept);
    chain6.set_type(ChainType::Filter);
    batch6.add(&chain6, MsgType::Add);

    for addr in blocks {
        match &addr {
            IpNetwork::V4(addr) => create_rule(
                &chain4,
                &mut batch4,
                NFPROTO_IPV4,
                nftnl::expr::NetworkHeaderField::Ipv4(nftnl::expr::Ipv4HeaderField::Daddr),
                addr.ip() & addr.mask(),
                addr.mask(),
            ),
            IpNetwork::V6(addr) => create_rule(
                &chain6,
                &mut batch6,
                NFPROTO_IPV6,
                nftnl::expr::NetworkHeaderField::Ipv6(nftnl::expr::Ipv6HeaderField::Daddr),
                addr.ip() & addr.mask(),
                addr.mask(),
            ),
        };
    }

    send(&batch4.finalize())?;
    send(&batch6.finalize())?;
    Ok(())
}

fn create_rule<'a>(
    chain: &'a Chain<'a>,
    batch: &'a mut Batch,
    nfproto: c_int,
    field: nftnl::expr::NetworkHeaderField,
    addr: impl ToSlice,
    mask: impl ToSlice + Clone,
) {
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta cgroup));
    rule.add_expr(&nft_expr!(cmp == cgroup::NET_CLS_CLASSID));
    rule.add_expr(&nft_expr!(meta nfproto));
    rule.add_expr(&nft_expr!(cmp == nfproto));
    rule.add_expr(&nftnl::expr::Payload::Network(field));
    rule.add_expr(&nftnl::expr::Bitwise::new(
        mask.clone(),
        vec![0u8; mask.to_slice().len()].as_slice(),
    ));
    rule.add_expr(&nftnl::expr::Cmp::new(nftnl::expr::CmpOp::Eq, addr));
    rule.add_expr(&nft_expr!(verdict drop));
    batch.add(&rule, MsgType::Add);
}

pub fn stop() -> Result<()> {
    delete_table(&c"ow2dropshiprs", nftnl::ProtoFamily::Ipv4)?;
    delete_table(&c"ow2dropshiprs", nftnl::ProtoFamily::Ipv6)?;
    Ok(())
}

fn delete_table(name: &impl AsRef<CStr>, family: ProtoFamily) -> Result<()> {
    let mut batch = Batch::new();
    let table = Table::new(name, family);

    batch.add(&table, MsgType::Del);
    let _ = send(&batch.finalize());
    Ok(())
}

fn send(batch: &FinalizedBatch) -> Result<()> {
    let socket = mnl::Socket::new(mnl::Bus::Netfilter)?;
    socket.send_all(batch)?;

    let portid = socket.portid();
    let size = nftnl::nft_nlmsg_maxsize() as usize;
    let mut buffer = vec![0; size];

    let seq = 0;
    while let Some(message) = socket_recv(&socket, &mut buffer[..size])? {
        match mnl::cb_run(message, seq, portid)? {
            mnl::CbResult::Stop => break,
            mnl::CbResult::Ok => {}
        }
    }
    Ok(())
}

fn socket_recv<'a>(socket: &mnl::Socket, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>> {
    let ret = socket.recv(buf)?;
    if ret > 0 {
        Ok(Some(&buf[..ret]))
    } else {
        Ok(None)
    }
}
