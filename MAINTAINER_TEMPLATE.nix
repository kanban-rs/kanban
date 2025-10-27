# Maintainer entry for nixpkgs/maintainers/maintainer-list.nix
# This is the entry to add for the kanban package maintainer
# Find your position alphabetically in the maintainer list and insert this entry

# Example: Replace GITHUB_ID with your actual GitHub user ID
# You can find it with: curl https://api.github.com/users/fulsomenko | jq '.id'

# Basic entry (recommended minimum):
fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = XXXXXXX;  # Your GitHub numeric ID
  name = "Max Emil Blomstervall";
};

# Extended entry (recommended for better maintenance):
fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = XXXXXXX;  # Your GitHub numeric ID
  name = "Max Emil Blomstervall";
  # GPG key fingerprint (optional but recommended for security)
  keys = [{
    fingerprint = "XXXXXXXX XXXXXXXX XXXXXXXX XXXXXXXX XXXXXXXX";
  }];
};

# Steps to populate:
# 1. Find your GitHub ID:
#    curl https://api.github.com/users/fulsomenko | jq '.id'
#
# 2. (Optional) Get your GPG fingerprint:
#    gpg --list-keys --keyid-format long your-email@example.com | grep pub
#
# 3. Find where 'fulsomenko' fits alphabetically in maintainers/maintainer-list.nix
#
# 4. Replace the entry with your actual values
#
# 5. Commit with message: "maintainers: add fulsomenko"
