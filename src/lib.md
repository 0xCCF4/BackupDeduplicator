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
└───┬───────┘ │       ││                                           
    │         │  ┌────▼▼────┐  ┌─────────────────┐                 
    │         │  │          │  │                 │                 
    │         └──► Analyze  ├──► Duplicate Sets  │                 
    │            │          │  │                 │                 
    │            └────┬┬────┘  └┬────────────────┘                 
    │                 ││        │                                  
    │         ┌───────┼┼────────┘                                  
    │         │       ││                                           
    │         │  ┌────▼▼────┐  ┌─────────────────┐                 
    │         │  |          │  │                 │                 
    │         └──►  Dedup   ├──► Change commands │                 
    │            |          │  │                 │                 
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
4. **Execute**: The tool executes the deduplication steps (Deleting/Hardlinking/...).

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
The analysis results are stored in a file with the following JSON format:
```plain
{
  "version": "V1",
  "entries": [
      ENTRY,
      ENTRY,
      ...
  ]
}
```


See `DupSetEntry` for the exact format of an entry. In short, it contains (JSON)
* File type
* Hash
* Size (if it is a directory: number of children, else the file size of one of the files)
* Conflicting Set (a set of all files that are duplicates of each other)

## Dedup
* Input: Duplicate sets
* Output: Required actions to deduplicate the files
* Execution: Fully automatic, no user interaction required.

Currently, there is just one deduplication strategy implemented: 
* **golden model**: delete all files outside of the "golden" directory that are also contained
withing the golden directory.

## Execute
* Input: Set of dedup actions
* Output: Deduplicated files
* Execution: Fully automatic, user interaction only on errors.

Implementation in progress.
