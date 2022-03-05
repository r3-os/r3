svgbobdoc::transform! { /**
```svgbob,[system_lifecycle]

                      Main thread
                      .-----------------------.
                      |                       |
                      |           *           |
                      |           |           |
                      |           v           |
  Boot phase          | Kernel initialization |
                      |           |           |
                      |           v           |
                      |     Startup hooks     |
                      |      "(user code)"    |
                      |           |           |
                      '-----------+-----------'
                                  |
  - - - - - - - - - - - - - - - - + - - - - - - - - - - - - - - - - -
                                  |
                                  | For each hardware thread...
                                  |
                                  +<------------------------------.
                                  |                               |
                                  v                               |
                      Determine the next thread                   |
                             to execute                           |
                                  |                               |
                                  |                               |
                                  v                               |
           .--------------+--------------+--------------+--- ...  |
           |              |         Int. |         Int. |         |
    Task 1 |       Task 2 |    handler 1 |    handler 2 |         |
     .-----+-----.  .-----+-----.  .-----+-----.  .-----+-----.   |
     |     |     |  |     |     |  |     |     |  |     |     |   |
     |     v     |  |     v     |  |     v     |  |     v     |   |
     | User code |  | User code |  | User code |  | User code |   |
     |     |     |  |     |     |  |     |     |  |     |     |   |
     |     |     |  |     |     |  |     |     |  |     |     |   |
     '-----+-----'  '-----+-----'  '-----+-----'  '-----+-----'   |
           |              |              |              |         |
           '--------------+--------------+--------------+---------'
              The thread exiting, preempted, or blocked
              or an interrupt taken
```
*/ }
