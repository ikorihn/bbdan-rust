# bbdan

CLI for Bicbucket admin

## Install

```shell
$ cargo install --path .
```

## Options

- `-u, --username` Bitbucket username
- `-p, --password` Bitbucket app password
- `-w, --workspace` Bitbucket workspace

## Commands

### `list`

List permissions for a repository.

```shell
$ bbdan list
```

### `copy`

Copy permissions of a project to another project.

```shell
$ bbdan copy project-A project-B
```

### `remove`

Select and remove permission of a repository.

```shell
$ bbdan remove
```
