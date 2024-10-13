# Trasher

[![crates.io](https://img.shields.io/crates/v/trasher)](https://crates.io/crates/trasher)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

Trasher is a small command-line utility that aims to replace `rm`.

It works by moving items to delete to a _trash directory_ instead of deleting them immediatly. As moving a single item is nearly instant (even when it's a large directory), while deleting items recursively can take quite a long time, Trasher is **faster** than `rm`, especially for large directories.

An optional fuzzy finder is included to restore items interactively.

## Usage

There are several actions available:

* `ls`: list items in the trash, use `-d / --details` to get the size and content of items
* `rm <path>`: move an item to the trash, use `-p / --permanently` to delete the item instead of moving it to the trash
* `unrm <name>`: restore an item in the current directory, use `--id` to provide an ID and `--to` to specify another restoration location
* `drop <name>`: permanently delete an item from the trash, use `--id` to provide an ID
* `path-of <name>`: get the path to an item inside the trash directory
* `trash-path`: get the path to the trash directory associated to the current mountpoint (depends on the shell's current directory)
* `empty`: remove all items from the trash
* `help`: display informations about this tool's usage

## How does it work

When an item is moved to the trash, its name is suffixed by its base64-encoded date of deletion.

For instance, when deleting an item named `my-files`, it will be moved to the trash directory under a name like:

```
my-files ^TrCxIAqzuA
```

This allows you to open the trash directory and see its content without using the Trasher binary. Also, Trasher doesn't use an index file, it only extracts informations from the files present in the trash, so you can move it to another drive without any problem, or even merge two trash directories into a single one!

This renaming also allows to delete multiple items with the same name without any conflict.

You can then then restore items from the trash by specifying their names. If multiple items have the same name, a list of items with the provided name will be displayed along with their ID, and you will be asked to specify the ID of the item you want to restore.

### External filesystems

The moving is actually performed by renaming the file, which is a lot faster than moving data around and gives exactly the same result. For external filesystems, a trash directory is created at the root of the filesystem. You can the use the `trash-path` subcommand to see the trash directory associated to the current folder, for instane:

```
cd ~/Downloads
trasher trash-path # /home/<username>/.trasher*

cd /mnt/somewhere/something
trasher trash-path # probably /mnt/somehwere
```

## Technical details

Removed items' name must be UTF-8-compliant, so invalid UTF-8 filenames will make the program fail unless `-a / --allow-invalid-utf8-item-names` flag is provided during deletion, which will result in converting the filename to a valid UTF-8 string lossily.

Trash item's name is composed of the original item's name, its removal date and time with nanosecond precision and timezone, which is then base64-encoded and acts as a unique identifier for this file (CPU speed isn't fast enough to allow two items to be deleted at the exact same nanosecond, much less two items which would happen to have the same name).

When restoring an item, if multiple trash items have the same name, the ID is required to know which file to restore.
