# Inner-workings
The tool is run in four stages:
```plain
   Input         Execution       Output                         
                    ││                          
┌───────────┐  ┌────▼▼────┐  ┌────────────┐     
│  Folder   │  │          │  │            │     
│   -file   ├──►  Build   ├──►  HashTree  │     
│   -file   │  │          │  │            │     
└─┬────┬────┘  └────┬┬────┘  └┬───────────┘     
  │    │            ││        │                 
  │    │    ┌───────┼┼────────┘                 
  │    │    │       ││                          
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
  │            │ Execute  ├──► Deduplicated files
  └────────────►          │                     
               └──────────┘                     
```
1. **Build**: The tools reads a folder and builds a hash tree of all files in it.
2. **Analyze**: The tool analyzes the hash tree and finds duplicate files.
3. **Dedup**: The tool determine which steps to take to deduplicate the files.
This can be done in a half automatic or manual way.
4. **Execute**: The tool executes the deduplication steps (Deleting/Hardlinking/...).

**Dedup** and **Execute** are in development and currently not (fully) implemented.
