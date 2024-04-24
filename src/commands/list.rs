use crate::{challenge::Challenge, fly, Config};
use anyhow::Result;
use colored::*;
use std::collections::HashMap;

enum MachineState {
    NotDeployed,
    Stopped,
    Started,
}

pub fn command(config: Config) -> Result<()> {
    let challs = Challenge::get_all(&config.chall_root)?;
    let machines = fly::list_machines(&config.fly.app_name)?
        .into_iter()
        .map(|machine| (machine.name, machine.state))
        .collect::<HashMap<String, String>>();
    for chall in challs {
        let deploy_status = chall
            .containers
            .iter()
            .map(|(name, _)| {
                let id = chall.container_id(&name);
                let state = match machines.get(&id) {
                    Some(state) => {
                        if state == "started" {
                            MachineState::Started
                        } else {
                            MachineState::Stopped
                        }
                    }

                    None => MachineState::NotDeployed,
                };
                format!(
                    "({} {})",
                    name,
                    match state {
                        MachineState::Started => "started".green(),
                        MachineState::Stopped => "stopped".white().dimmed(),
                        MachineState::NotDeployed => "not deployed".red(),
                    }
                    .bold()
                )
            })
            .collect::<String>();
        println!("{} - {}", chall.id, deploy_status);
    }
    return Ok(());
}
