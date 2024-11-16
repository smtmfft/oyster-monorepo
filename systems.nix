{
  systems = [
    {
      system = "x86_64-linux";
      rust_target = "x86_64-unknown-linux-musl";
      static = true;
    }
    {
      system = "aarch64-linux";
      rust_target = "aarch64-unknown-linux-musl";
      static = true;
    }
    {
      system = "x86_64-darwin";
      rust_target = "x86_64-apple-darwin";
      static = false;
    }
    {
      system = "aarch64-darwin";
      rust_target = "aarch64-apple-darwin";
      static = false;
    }
  ];
  forSystems = systems: f:
    builtins.listToAttrs (map (
        system: {
          name = system.system;
          value = f system;
        }
      )
      systems);
}
