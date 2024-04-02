# Inner-workings
The tool is run in four stages:
```plain
     Input         Execution       Output         
┌───────────┐         ┌┐                          
│ HashTree  ◄─────────┼┼────────┐                 
│           │         ││        │                 
│(optional) ├──┐ ┌────▼▼────┐  ┌┴───────────────┐ 
└───────────┘  └─►          │  │                │ 
                 │  Build   ├──►  HashTree      │ 
┌───────────┐  ┌─►          │  │                │ 
│  Folder   ├──┘ └────┬┬────┘  └┬───────────────┘ 
│   -file   │         ││        │                 
│   -file   │ ┌───────┼┼────────┘                 
└───┬────┬──┘ │       ││                          
    │    │    │  ┌────▼▼────┐  ┌────────────────┐ 
    │    │    │  │          │  │                │ 
    │    │    └──► Analyze  ├──► Duplicate Sets │ 
    │    │       │          │  │                │ 
    │    │       └────┬┬────┘  └┬───────────────┘ 
    │    │            ││        │                 
    │    │    ┌───────┼┼────────┘                 
    │    │    │       ││                          
    │    │    │  ┌────▼▼────┐  ┌─────────────────┐
    │    │    └──►          │  │                 │
    │    │       │  Dedup   ├──► Change commands │
    │    └───────►          │  │                 │
    │            └────┬┬────┘  └┬────────────────┘
    │                 ││        │                 
    │         ┌───────┼┼────────┘                 
    │         │       ││                          
    │         │  ┌────▼▼────┐                     
    │         └──►          │                     
    │            │ Execute  ├──►Deduplicated files
    └────────────►          │                     
                 └──────────┘                     
```
1. **Build**: The tools reads a folder and builds a hash tree of all files in it.
2. **Analyze**: The tool analyzes the hash tree and finds duplicate files.
3. **Dedup**: The tool determine which steps to take to deduplicate the files.
This can be done in a half automatic or manual way.
4. **Execute**: The tool executes the deduplication steps (Deleting/Hardlinking/...).

**Dedup** and **Execute** are in development and currently not (fully) implemented.

## Build
* Input: Folder with files, Hashtree (optional) to update or continue from.
* Output: HashTree
* Execution: Fully automatic, no user interaction required, multithreaded.

### HashTree file format
The HashTree is stored in a file with the following format:
```plain
HEADER [newline]
ENTRY [newline]
ENTRY [newline]
...
```
See `SaveFileEntry` for the exact format of an entry. In short, it contains
every information about an analyzed file or directory that is needed for later
stages.
* File path
* File type
* Last modified time
* File size
* Hash of the file
* Children hashes (if it is a directory)

While analyzing entries are only appended to the file. After the analysis is
done, the file is fed into the `clean` command that removes all entries that
are outdated or do not exist anymore.

The `clean` command can also be run manually.

## Analyze
* Input: HashTree
* Output: Duplicate sets
* Execution: Fully automatic, no user interaction required, multithreaded file parsing,
  single-threaded duplication detection.

### Analysis results
The analysis results are stored in a file with the following format:
```plain
