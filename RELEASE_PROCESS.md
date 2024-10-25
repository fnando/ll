# Release Process

## macOS

1. Install dependencies with `brew bundle install`
2. Update the version on `Cargo.toml`
3. Run `bin/build`
4. Make a commit (e.g. `Bump up version (v0.0.1)`)
5. Create a tag with `git tag v0.0.1`
6. Push with `git push && git push --tags`
7. Go to https://github.com/fnando/ll/releases/new and create a new release out
   of this tag
8. Upload the files from `build/v0.0.1/*.tar.gz`
9. Copy the formula output and paste it on `fnando/homebrew-tap/ll.rb`, make a
   commit and push it. You can locally test it by running `brew install ll.rb`
