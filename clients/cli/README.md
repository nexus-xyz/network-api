# Network CLI

The command line interface (CLI) lets you run a Nexus prover node.
It is the highest-performance option for proving.

## Prerequisites

If you don't have these dependencies already, install them first.

### Linux

```
sudo apt update
sudo apt upgrade
sudo apt install build-essential pkg-config libssl-dev git-all
```

### macOS

If you have [installed Homebrew](https://brew.sh/) to manage packages on OS X,
run this command to install Git.

```
brew install git
```

### Windows

[Install WSL](https://learn.microsoft.com/en-us/windows/wsl/install),
then see Linux instructions above.

## Quick start

```
curl https://cli.nexus.xyz/ | sh
```

If you do not already have Rust, you will be prompted to install it.

## Terms of Use

Use of the CLI is subject to the [Terms of Use](https://nexus.xyz/terms-of-use).
The first time you run it, it prompts you to accept the terms. To accept the terms
noninteractively (for example, in a continuous integration environment),
add `NONINTERACTIVE=1` before `sh`.

## Turning out the nexus process to run as a service (optional)

Note: This will help anyone who runs cli whose process is stopped after a period of time, will be automatically restarted by systemd and continue the process without having to do it manually again.

- Assume i'm staying at /root path (can be anywhere on your machine)
- Download file script: curl https://cli.nexus.xyz/install.sh > nexus.sh

#### Update `nexus.sh` file
- To accept the terms noninteractively
- Specify absolute path of cargo and rustc

```
sed -i 's|rustc|/root/.cargo/bin/rustc|g' nexus.sh
sed -i 's|cargo|/root/.cargo/bin/cargo|g' nexus.sh

sed -i '5i NONINTERACTIVE=1' nexus.sh
```

### Create a systemd service 
`nano /etc/systemd/system/nexus.service`

and input this configuration
```
[Unit]
Description=Nexus Process
After=network.target

[Service]
ExecStart=/root/nexus.sh  # <==== make sure to change this file location to match where you put the file
Restart=on-failure
RestartSec=5
RestartPreventExitStatus=127
SuccessExitStatus=127
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

save and exit
```
ctrl + o (save)
ctrl + x (exit the editor)
```

### Reload the systemd daemon
```
sudo systemctl daemon-reload
```

### Start and Enable the Service
```
sudo systemctl start nexus.service
sudo systemctl enable nexus.service
```

### Check the Status (waiting for a few minutes to let it build the code)
```
sudo systemctl status nexus.service
```
something like this, this is ok
```
● nexus.service - Nexus Process
     Loaded: loaded (/etc/systemd/system/nexus.service; enabled; vendor preset: enabled)
     Active: active (running) since Sat 2024-10-26 16:23:13 CEST; 2min 16s ago
   Main PID: 951443 (nexus.sh)
      Tasks: 34 (limit: 77071)
     Memory: 1.2G
        CPU: 1min 14.009s
     CGroup: /system.slice/nexus.service
             ├─951443 /bin/sh /root/nexus/nexus.sh
             └─951458 target/release/prover beta.orchestrator.nexus.xyz

Oct 26 16:23:40 vmi2192653.contaboserver.net nexus.sh[951458]: Proved step 14 at 3.85 proof cycles/sec.
Oct 26 16:23:41 vmi2192653.contaboserver.net nexus.sh[951458]: Proved step 15 at 4.16 proof cycles/sec.
Oct 26 16:23:42 vmi2192653.contaboserver.net nexus.sh[951458]: Proved step 16 at 3.80 proof cycles/sec.
```


### Monitor the Logs
```
journalctl -u nexus.service -f

ctrl + c to exit
```

## Known issues

* Only the latest version of the CLI is currently supported.
* Prebuilt binaries are not yet available.
* Counting cycles proved is not yet available in the CLI.
* Only proving is supported. Submitting programs to the network is in private beta.
To request an API key, contact us at growth@nexus.xyz.

## Resources

* [Network FAQ](https://nexus.xyz/network#network-faqs)
* [Discord channel](https://discord.gg/nexus-xyz)
