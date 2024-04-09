# Inner-workings
The tool is run in four stages:
```plain
     Input         Execution       Output                          
┌───────────┐         ┌┐                                           
│ HashTree  ◄─────────┼┼────────┐                                  
│           │         ││        │                                  
│(optional) ├──┐ ┌────▼▼────┐  ┌┴────────────────┐                 
└───────────┘  └─►          │  │                 │                 
                 │  Build   ├──►  HashTree       │                 
┌───────────┐  ┌─►          │  │                 │                 
│  Folder   ├──┘ └────┬┬────┘  └┬────────────────┘                 
│   -file   │         ││        │                                  
│   -file   │ ┌───────┼┼────────┘                                  
└───┬────┬──┘ │       ││                                           
    │    │    │  ┌────▼▼────┐  ┌─────────────────┐                 
    │    │    │  │          │  │                 │                 
    │    │    └──► Analyze  ├──► Duplicate Sets  │                 
    │    │       │          │  │                 │                 
    │    │       └────┬┬────┘  └┬────────────────┘                 
    │    │            ││        │      Basic functionality complete
----│----│----┌───────┼┼────────┘----------------------------------
    │    │    │       ││                 Implementation in progress
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
See `HashTreeFileEntry` for the exact format of an entry. In short, it contains
every information about an analyzed file or directory that is needed for later
stages (JSON):
* File path
* File type
* Last modified time
* File size
* Hash of the file
* Children hashes (if it is a directory)

While analyzing entries are only appended to the file. After the analysis is
done, the file is fed into the `clean` command that removes all entries that
are outdated or do not exist anymore, rewriting the entire file (but only shrinking it).

The `clean` command can also be run manually.

## Analyze
* Input: HashTree
* Output: Duplicate sets
* Execution: Fully automatic, no user interaction required, multithreaded file parsing,
  single-threaded duplication detection.

### Analysis results
The analysis results are stored in a file with the following format:
```plain
[ENTRY] [newline]
[ENTRY] [newline]
...
```
See `ResultEntry` for the exact format of an entry. In short, it contains (JSON)
* File type
* Hash
* Size (0 if it is a directory, else the file size of one of the files)
* Conflicting Set (a set of all files that are duplicates of each other)

## Dedup
* Input: Duplicate sets
* Output: Set of commands to execute to deduplicate the files
* Execution: Manual or half-automatic, user interaction required.

Implementation in progress. To the current date the duplicate sets
must be manually processed.

## Execute
* Input: Set of commands
* Output: Deduplicated files
* Execution: Fully automatic, user interaction only on errors.

Implementation in progress.
