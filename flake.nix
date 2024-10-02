{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };

  outputs = { self, fenix, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system: 
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenix' = fenix.packages.${system};
        rust-pack = (fenix'.complete.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
          "rust-std"
          "rust-docs"
          "rust-analyzer"
        ]);

      in {
        devShell = with pkgs; mkShell ({
          packages = [ 
            rust-pack
            openssl
            pkg-config
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath [ openssl ];
        });
      }
    );
}
