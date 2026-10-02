# Meerkateer Controller Developer Preview notice

Please read this notice before installing or connecting a computer.

1. **Developer Preview.** This Controller is pre-release software. Interfaces, configuration,
   package formats, and behavior may change before Meerkateer 1.0.
2. **No warranty.** The software is provided under the Apache License 2.0 on an **AS IS** basis,
   without warranties or conditions of any kind. This notice does not replace or modify the
   Apache License 2.0.
3. **Unsigned preview packages.** Direct-download preview packages are not code-signed. Download
   them only from the official Meerkateer GitHub release and verify the adjacent SHA-256 checksum
   before installation.
4. **Administrative changes.** Installation requires administrator/root authority. Setup creates a
   protected local configuration containing a machine credential and registers a background
   Windows startup task or Linux systemd service.
5. **Outbound telemetry.** After enrollment, the Controller sends a mandatory heartbeat and only
   the CPU, memory, disk totals, and exact process-running signals selected during setup to the
   Meerkateer API address entered by the operator. Review the destination and signal allowlist
   before connecting the computer.
6. **Operator responsibility.** Install the Controller only on computers you are authorized to
   administer. Protect the Meerkateer API with HTTPS, firewall rules, access controls, backups, and
   normal operating-system security. Do not expose a preview installation directly to untrusted
   networks.
7. **No remote shell.** This preview opens outbound connections only and does not provide an
   arbitrary remote shell or accept arbitrary remote commands.

By continuing with setup, you confirm that you are authorized to administer this computer, have
reviewed the destination and collected signals, and understand the Developer Preview risks above.
