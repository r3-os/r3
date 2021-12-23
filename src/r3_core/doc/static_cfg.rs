svgbobdoc::transform! { /**
```svgbob,[static_cfg]
                            &mut Cfg<C>
    const fn configure_app <------------ "r3_kernel::build!(SystemTraits,"
          "(...) -> Objects"------------> "configure_app => Objects)"
                              Objects           |  |
 "fn normal_code()"                             |  |
     |    |                                     |  | Generate code in the
     |    v                             Objects |  | application crate
     | .-------------------------.              |  |
     | | static COTTAGE: Objects |<-------------'  |
     | '-------------------------'                 |
     |                                             v
     | Through app-side API            .---------------------------------.
     v                                 |   .-------------------------.   |
  .---------------------------------.  |   | static TASK_CB_POOL ... |   |
  | r3_kernel::System<SystemTraits> |  |   '------------+------------'   |
  |                                 |  |                |                |
  |                .--------------. |  |  .-------------+-------------.  |
  |                | SystemTraits +-|--|--+ impl ... for SystemTraits |  |
  |                '--------------' |  |  '---------------------------'  |
  '---------------------------------'  '---------------------------------'
```
*/ }
