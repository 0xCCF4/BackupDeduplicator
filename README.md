# Backup Deduplicator
A tool to deduplicate backups. It builds a hash tree of all files and folders
in a target directory. Optionally also traversing into archives like zip or tar
files (feature in development). The hash tree is then used to find duplicate
files and folders. The output is a minimal duplicated set. Therefore, the tool
discovers entire duplicated folder structures and not just single files.

Backup Deduplicator solves the problem of having multiple backups of the same
data, whereas some parts of the data are duplicated. Duplicates can be reviewed
and removed to save disk space (feature in development).

## Features
* **Multi threading**: The tool is able to use multiple threads to speed up the
  hash calculation process.
* **Pause and resume**: The tool can be paused (killed) and resumed at any time. The
  current state is saved to disk and can be loaded later. This is useful for long
  analysis processes (large directories).
* **Cache and resume**: The tool can be run at a later point reusing the cache from
  a previous run. This is useful for re-analyzing a directory after some changes
  have been made.
* **Follow or not follow symlinks**: The tool can be configured to follow symlinks
  or not.
* **Hash collision robustness**: The tool uses hashes to detect duplicates.
  There is a probability of hash collisions. For the final duplicate detection,
  not only the hash but also the file size and file types are compared to reduce
  the probability of false positives. When choosing a weak hash function (with many
  false duplicates), the tool may run slower.

### Planned-features
* **Archive support**: The tool will be able to traverse into archives like zip
  or tar files to find duplicated structures there.
* **CUI**: A graphical command line interface will be added to allow easy duplicate
  processing (removal/excluding/...).
* **Multi machine analysis**: The tool will be able to analyze a (shared) directory 
  on multiple machines in parallel to speed up the analysis process.
* **Merge**: The tool will be able to merge analysis files such that analysis results
  from different machines can be combined.
* **Hardlinks**: The tool will be able to detect hardlinks and treat them as not duplicates
  (if set via flags).
* **Evaluation modes**: Different analysis modes. Allowing for example to set a
  directory of truth (archival directory) to compare against. Every file/folder already
  in the truth directory, found elsewhere will be marked as duplicate to remove. 

## Usage
The tool is a command line tool. There are two stages: `build` and `analyze`.
 1. **Build**: The tool builds a hash tree of the target directory. This is done
    by running `backup-deduplicator build [OPTIONS] <target>`. The hash tree is saved to
    disk and is used by the next stage.
2. **Analyze**: The tool analyzes the hash tree to find duplicates. This is done
    by running `backup-deduplicator analyze [OPTIONS]`. The tool will output a list of
    duplicated structures to an analysis result file.

### Build
Exemplary usage to build a hash tree of a directory:
```bash
backup-deduplicator --threads 16 build -w /parent -o /parent/hash.bdd /parent/target
```
This will build a hash tree of the directory `/path/to/parent/target` and save it to
`hash.bdd` in the parent directory. The tool will use 16 threads to split the hash
calculation work.

### Analyze
Exemplary usage to analyze a hash tree:
```bash
backup-deduplicator analyze -o /parent/analysis.bdd /parent/hash.bdd
```
This will analyze the hash tree in `hash.bdd` and save the analysis result to `analysis.bdd`.
The analysis file will then contain a list of JSON objects (one per line),
each representing a found duplicated structure.

Further processing with this tool is in development.

## Installation
The tool is written in Rust, and can be installed using `cargo`:
```bash
cargo install backup-deduplicator
```

## Contribution
Contributions to PhotoSort are welcome! If you have a feature request,
bug report, or want to contribute to the code, please open an
issue or a pull request.

## License
This project is licensed under the GPLv3 license. See the LICENSE file for details.
