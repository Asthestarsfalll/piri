{
  description = "Piri is a high-performance Niri extension tool built with Rust.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "piri";
            version = "0.1.8";

            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];

            buildInputs = [
              pkgs.wayland
              pkgs.cairo
              pkgs.glib
            ];

            meta = {
              description = "Piri is a high-performance Niri extension tool built with Rust.";
              homepage = "https://github.com/Asthestarsfalll/piri";
              license = nixpkgs.lib.licenses.mit;
              platforms = nixpkgs.lib.platforms.linux;
              mainProgram = "piri";
            };
          };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/piri";
        };
      });

      nixosModules = {
        piri =
          { config, pkgs, lib, ... }:
          let
            cfg = config.services.piri;
          in
          {
            options.services.piri = {
              enable = lib.mkEnableOption "piri daemon";
              package = lib.mkPackageOption self.packages.${pkgs.stdenv.hostPlatform.system} "default" { };
            };

            config = lib.mkIf cfg.enable {
              environment.systemPackages = [ cfg.package ];

              systemd.user.services.piri = {
                description = "piri daemon";
                wantedBy = [ "graphical-session.target" ];
                partOf = [ "graphical-session.target" ];
                wants = [ "graphical-session.target" ];
                after = [ "graphical-session.target" ];
                enableDefaultPath = false;
                serviceConfig = {
                  Type = "simple";
                  Restart = "on-failure";
                  ExecStart = "${lib.getExe cfg.package} daemon";
                };
              };
            };
          };

        default = self.nixosModules.piri;
      };

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];

            packages = [
              pkgs.rustfmt
              pkgs.clippy
              pkgs.cargo-watch
              pkgs.rust-analyzer
            ];

            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        });
    };
}
