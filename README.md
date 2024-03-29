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
* `unrm-ui`: restore an item interactively (a fuzzy finder will be displayed)
* `drop <name>`: permanently delete an item from the trash, use `--id` to provide an ID
* `clear`: remove all items from the trash

## How does it work

When an item is moved to the trash, its name is suffixed by its date of deletion and by an ID which is computed using a double CRC (see [technical details](#technical-details)).

For instance, when deleting an item named `my-files`, it will be moved to the trash directory under a name like:

```
my-files [@ 2020.08.03_11h36m36s.093347700+0200] {IBuc}
```

This allows you to open the trash directory and see its content without using the Trasher binary. Also, Trasher doesn't use an index file, it only extracts informations from the files present in the trash, so you can move it to another drive without any problem, or even merge two trash directories into a single one!

This renaming also allows to delete multiple items with the same name without any conflict.

You can then then restore items from the trash by specifying their names. If multiple items have the same name, a list of items with the provided name will be displayed along with their ID, and you will be asked to specify the ID of the item you want to restore.

### External filesystems

The moving is actually performed by renaming the file, which is a lot faster than moving data around and gives exactly the same result. For external filesystems, a trash directory is created at the root of the filesystem.

## Technical details

Removed items' name must be UTF-8-compliant, so invalid UTF-8 filenames will make the program fail unless `-a / --allow-invalid-utf8-item-names` flag is provided during deletion, which will result in converting the filename to a valid UTF-8 string lossily.

Trash item's name is composed of the original item's name, its removal date and exact time in nanoseconds with timezone, as well as an ID which is a CRC on 24 bits of the deletion date.

CRC has been chosen as it's extremely fast and there's extremely low risks of collision between two different dates with a CRC on 24 bits (unless an item with the same name is deleted hundreds of thousands of times).

When removing an item, if multiple trash items have the same name, the ID is required _along with the name_, so the filename doesn't need to be CRC-ed and so we avoid all risks of collisions with filenames, which can have an enormous number of different values.
