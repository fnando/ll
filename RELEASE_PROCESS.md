# Release Process

1. Install dependencies with `brew bundle install`
2. Update the version on `Cargo.toml`
3. Build executables (see below)
4. Make a commit (e.g. `Bump up version (v0.0.1)`)
5. Create a tag with `git tag v0.0.1`
6. Push with `git push && git push --tags`
7. Go to https://github.com/fnando/ll/releases/new and create a new release out
   of this tag
8. Upload the files from `build/v0.0.1/*.{tar.gz,deb}`
9. Make it available on homebrew: `fnando/homebrew-tap/ll.rb`.

## Build executables

### macOS

1. Copy
   `/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`
   to the root folder of this repo.
2. Build the docker image with `make image`.
3. Build the executables with `make dist`.
