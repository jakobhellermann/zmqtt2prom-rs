{
  description = "zmqtt2prom-rs - MQTT to Prometheus bridge";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = nixpkgs.lib.genAttrs supportedSystems;

      mkFor =
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.mkLib pkgs;
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
          };
          cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { pname = "zmqtt2prom-rs-deps"; });
        in
        {
          inherit pkgs craneLib;
          zmqtt2prom-rs = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          zmqtt2prom-rs-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );
          zmqtt2prom-rs-test = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });
        };

      forAllSystems = eachSystem mkFor;
    in
    {
      packages = eachSystem (system: {
        default = forAllSystems.${system}.zmqtt2prom-rs;
        zmqtt2prom-rs = forAllSystems.${system}.zmqtt2prom-rs;
      });

      apps = eachSystem (system: {
        default = {
          type = "app";
          program = "${forAllSystems.${system}.zmqtt2prom-rs}/bin/zmqtt2prom";
        };
      });

      checks = eachSystem (system: {
        inherit (forAllSystems.${system}) zmqtt2prom-rs zmqtt2prom-rs-clippy zmqtt2prom-rs-test;
      });

      devShells = eachSystem (
        system:
        let
          f = forAllSystems.${system};
        in
        {
          default = f.craneLib.devShell {
            checks = self.checks.${system};
            inputsFrom = [ f.zmqtt2prom-rs ];
            packages = with f.pkgs; [
              rust-analyzer
              cargo-watch
              cargo-edit
            ];
          };
        }
      );
    };
}
