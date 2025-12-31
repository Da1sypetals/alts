Implement a version control software `alts`.

alts init dir_name
- check dir_name existense UNDER THIS DIR. if dir_name is not under current dir, report error.
- write to .alts/alts.toml under current dir, record the target dir name


alts checkpoint
- copy recursively dir_name to dir_name suffixed YYYY_MM_DD_HH_MM_SS under .alts/
    - log the copy process
- if not initialized, report an error
- if target is empty dir, or not exist, report an error

alts checkpoint <name>
- the same, but with name specified.
- make sure the name is not used.


use anyhow, log, env_logger (hardcode the log level to info), clap, toml.
use whatever lib you find convenient. DO NOT KEEP MINIMAL DEPENDENCY.

---

add these commands:

alts ls / alts list
list all checkpoints. 
Also every checkpoint should be added to index (toml) when doing checkpoint.
A checkpoint in index should also contain timestamp.


alts prune
remove all unfound checkpoints in the index.
show if each checkpoint is found or not to the user.


---

add 

alts info

show the metadata in a human readable way.
show size in human readable way in `alts info`. round to 1/100