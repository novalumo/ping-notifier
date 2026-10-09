{
  description = "Menu bar app that notifies you of ping timeouts";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      # アプリ自体は macOS / Windows 向けなので、パッケージは macOS だけに出す
      darwinSystems = [
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      forDarwinSystems = nixpkgs.lib.genAttrs darwinSystems;
    in
    {
      packages = forDarwinSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = self.packages.${system}.ping-notifier;
          ping-notifier = pkgs.callPackage ./nix/package.nix { };
        }
      );

      apps = forDarwinSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/ping-notifier";
        };
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              cargo-bundle
              librsvg
              imagemagick
            ];
          };
        }
      );
    };
}
