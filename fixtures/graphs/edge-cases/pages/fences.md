- Backtick fence
  ```md
  - not a block
  key:: not a property
  [[x]] #nope TODO nothing
  ```
- Tilde fence
  ~~~
  - still not a block
  ~~~
- Long fence
  ````
  ```
  - inner
  ```
  ````
- After fences [[real]]
- Fence on the first line ```not a fence``` [[inline]]
- Unclosed fence
  ```
  - swallowed? no: a bullet at the same or lower indent ends it
- Block after unclosed fence
