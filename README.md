# Tyr: A Policy Description Language

```
tag this;
tag that;
taggroup those = this, that;

struct MetaData [this] {
    field f1: int,
    field f2: str,
};

struct Data [] {
    field meta: MetaData,
    field f2: str,
};

action read_data(Data);
action do_nothing();
actiongroup all = read_data, do_nothing;

rules read_data(Data data) fallback deny {
    allow when data.meta.f1 > 4 and data.meta contains this;
    deny always;
    apply [that] always;
};
```
