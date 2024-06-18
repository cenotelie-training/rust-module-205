# Projet Playground Rust avec WebAssembly

Vous pouvez démarrer ce projet à n'importe quelle étape de cette feuille de route.
Un tag git de la forme `step{i}` où `i` est le numéro de l'étape est fourni pour facilement commencer de n'importe où.
Par example pour reprendre à partir de l'étape 5, exécutez :

```bash
git checkout step5
```

Pour avoir le projet dans sa forme finale, exécutez :

```git
git checkout step16
```



# Présentation

Le but de ce projet est de réaliser un petit service web qui peut exécuter un sample de code Rust et nous retourner le résultat,
pour l'intégrer par exemple dans un playground en ligne comme [Rust playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021).
Pour réaliser cela, nous allons nous appuyer sur WebAssembly pour contrôler l'exécution de ce code arbitraire par nature.
L'idée est donc de :

- compiler un sample de code vers WebAsssembly, avec les interfaces systèmes WASI,
- exécuter le bundle WebAssembly produit en contrôlant l'exposition des interfaces systèmes,
- exposer ce service avec un serveur HTTP.

Nous allons utiliser :

- [Wasmtime](https://wasmtime.dev/) comme runtime WebAssembly à embarquer dans notre projet
- [axum](https://github.com/tokio-rs/axum) comme serveur HTTP

Pour que cela fonctionne, il nous faudra installer une cible supplémentaire pour notre chaîne Rust:

```bash
rustup target install wasm32-wasi
```



# Compiler un sample vers WebAssembly

Le but dans un premier temps est d'écrire une fonction `async` qui :
- prend en entrée le sample de code Rust d'intérêt
- le compile vers la cible `wasm32-wasi` en invoquant `rustc`
- et nous retourne le résultat de la compilation.


## Step 1 : Ecrire le sample complet à compiler dans un fichier

Dans une fonction `compile_input` avec la signature suivante :

```rust
async fn compile_input(sample: &str) -> Result<BuildResult, anyhow::Error> {}
```

Ecrire le sample de code à compiler dans un fichier.
A la fin du sample, ajouter un stub pour nous servir de point d'entré plus tard :

```rust
#[no_mangle]
pub extern "C" fn __entry() { let _ = main(); }
//                ^^^^^^^ notre point d'entré connu
```


## Step 2 : Lire le sample depuis stdin

Dans la fonction `main`, lire le sample depuis stdin et le passer à `compile_input` afin de pouvoir tester.

Tester que tout fonctionne avec la commande:

```bash
cat hello.rs | cargo run
```


## Step 3 : Compiler le sample

Compléter la fonction `compile_input` pour maintenant compiler le sample vers la cible `wasm32-wasi` et retourner la sortie de `rustc` en cas d'erreur.

La ligne de commande de `rustc` à invoquer est la suivante :

```bash
rustc --target wasm32-wasi --crate-type cdylib hello.rs
#                                              ^^^^^^^^
#                          fichier contenant le sample
```

Compléter l'usage dans la fonction `main` et exécuter.

```bash
cat hello.rs | cargo run
```

Vérifier que vous avez bien un bundle `.wasm` produit.



# Exécuter le bundle wasm produit

Dans cette partie, l'objectif est de charger le bundle compilé précédemment
et de l'exécuter.
Pour cela nous allons utiliser [Wasmtime](https://wasmtime.dev/) comme runtime afin de nous permettre de contrôler quel sera l'accès de ce bundle au système hôte.


## Step 4 : Fonction d'exécution

Définir une fonction `execute_payload` avec la signature suivante :

```rust
async fn execute_payload(wasm_file: &str) -> Result<(), anyhow::Error> {}
```

et l'appeler depuis `main`.


## Step 5 : Initialiser un runtime

L'objectif est d'arriver à créer un objet [Store](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html) représentant l'état interne d'une instance du runtime, et d'initialiser celui-ci avec pour exposer certaines API systèmes.

- Créer un object [Config](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html).
  - activer le support de l'asynchrone avec `async_support`.
- Avec la config, créer un objet [Engine](https://docs.rs/wasmtime/latest/wasmtime/struct.Engine.html).
- Créer un contexte d'exécution avec la configuration par défaut pour WASI P1 avec [WasiCtxBuilder::build_p1](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.build_p1)
- Dans une nouvelle structure de données représentant l'état interne de notre runtime, encapsuler le contexte précédent.
- Créer un [Store](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html) avec l'engine et l'instance de notre structure interne (argument `data`).
- Créer un [Linker](https://docs.rs/wasmtime/latest/wasmtime/struct.Linker.html) en utilisant l'engine.
- Ajouter les bindings WASI P1 au linker avec [add_to_linker_async](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/preview1/fn.add_to_linker_async.html).


## Step 6 : Chargement et exécution du bundle

Charger le bundle généré sous la forme d'un [Module](https://docs.rs/wasmtime/latest/wasmtime/struct.Module.html) avec [Module::from_file](https://docs.rs/wasmtime/latest/wasmtime/struct.Module.html#method.from_file).

L'instancier avec le linker en utilisant [Linker::instantiate_async](https://docs.rs/wasmtime/latest/wasmtime/struct.Linker.html#method.instantiate_async).
Cette méthode produit une [Instance](https://docs.rs/wasmtime/latest/wasmtime/struct.Instance.html).

Sur l'instance utiliser [Instance::get_typed_func](https://docs.rs/wasmtime/latest/wasmtime/struct.Instance.html#method.get_typed_func) pour récupérer notre point d'entré `__entry`.

Invoquer le point d'entré avec [TypedFunc::call_async](https://docs.rs/wasmtime/latest/wasmtime/struct.TypedFunc.html#method.call_async).

Pour l'instant nous ne récupérons pas la sortie du sample, toutes les APIs systèmes sont désactivées.


## Step 7 : Sortie du bundle

Modifier la construction du contexte WASI P1 avant [WasiCtxBuilder::build_p1](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.build_p1) pour faire hériter le contexte d'exécution du bundle des flux stdout et stderr de notre processus en utilisant [WasiCtxBuilder::inherit_stdout](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.inherit_stdout) et [WasiCtxBuilder::inherit_stderr](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.inherit_stderr).

Nous devrions voir la sortie du bundle dans la console maintenant en exécutant

```bash
cat hello.rs | cargo run
```



# Capture de stdout et stderr

Le but étant à terme de retourner la sortie du sample, il nous faut capturer les flux stdout et stderr plutôt que de directement écrire sur ceux de notre processus.
Pour cela, il faudra utiliser [WasiCtxBuilder::stdout](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.stdout) et [WasiCtxBuilder::stderr](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.stderr) pour donner les flux vers lequel le bundle devra écrire.


## Step 8 : Flux custom

Pour commencer, nous allons définir notre propre flux qui consistera simplement à accumuler le contenu dans un object `String`.

Définir une structure `MyStream` qui contient un objet `String` encapsulé de telle sorte à ce qu'il puisse être partagé et muté dans un contexte multi-thread ([Arc](https://doc.rust-lang.org/std/sync/struct.Arc.html) et [Mutex](https://doc.rust-lang.org/std/sync/struct.Mutex.html)).

Dériver [Clone](https://doc.rust-lang.org/std/clone/trait.Clone.html) sur la structure.


## Step 9 : Implémentation de `StdoutStream`

Implémenter le trait [StdoutStream](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/trait.StdoutStream.html) sur `MyStream`.

L'implémentation de [StdoutStream::stream](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/trait.StdoutStream.html#tymethod.stream) sera simplement de retourner un clone de `MyStream` boxé.

Implémenter [Subscribe](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/trait.Subscribe.html) pour `MyStream`. Il suffit de tout le temps retourner `Box::pin(std::future::ready(()))`.

Finalement, implémenter aussi [HostOutputStream](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/trait.HostOutputStream.html) pour `MyStream`. L'implémentation de [HostOutputStream::write](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/trait.HostOutputStream.html#tymethod.write) consistera simplement à pousser dans données dans le buffer encapsulé.


## Step 10 : Utiliser `MyStream` pour le contexte WASI P1

Dans la configuration du contexte WASI P1 avec [WasiCtxBuilder::build_p1](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.build_p1), configurer les flux stdout et stderr en utilisant [WasiCtxBuilder::stdout](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.stdout) et [WasiCtxBuilder::stderr](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html#method.stderr) en passant une nouvelle instance de notre structure `MyStream`.

On veillera à garder un clone à côté pour accéder aux buffers après.


## Step 11 : Retourner la sortie du sample

En utilisant les clones de `Arc` gardé localement, récupérer le contenu des buffers pour stdout et stderr et les retourner de `execute_payload` en cas de succès.

Dans la fonction `main`, récupérer ces valeurs et les afficher dans la console.



# Exposition avec `axum`

Maintenant que la fonctionnalité est réalisée, nous allons l'exposer sous la forme d'une API web avec [axum](https://github.com/tokio-rs/axum).


## Step 12 : Hello world avec `axum`

Dans la fonction main, utiliser [axum::serve](https://docs.rs/axum/latest/axum/serve/fn.serve.html#examples) pour servir une route avec un hello world.

Lancer l'application et tester en appelant la route dans un navigateur par exemple ou avec curl:

```bash
curl http://localhost:3000/hello
```


## Step 13 : Route `execute` pour exécuter un sample

Définir une structure `ExecutionOutput` qui va contenir la sortie de l'exécution d'un sample. Il conviendra de dériver [Serialize](https://docs.rs/serde/latest/serde/trait.Serialize.html) pour sérialiser en JSON ensuite.

Ajouter une fonction `async` qui servira de route pour exécuter un sample de code Rust dans le body de la requête.
- prendre un argument de type `String` pour récupérer le body de la requête.
- utilisez `ExecutionOutput` en retour encapsulé avec [Json](https://docs.rs/axum/latest/axum/struct.Json.html).
- vous pouvez retourner un `Result` pour gérer les cas d'erreur.

Enregistrer la route sur le routeur axum avec une méthode `post` pour le chemin `/execute`.

Tester avec la commande :

```bash
curl -X POST -H "Content-Type: rext/plain" -d 'fn main() { println!("hello world!"); }' "http://localhost:8000/execute"
```



# Robustification

Pour l'instant les samples Rust que nous compilons et exécutons n'ont pas accès
aux interfaces du système. Pour cela il conviendra de les exposer lors de la création
du contexte avec [WasiCtxBuilder](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html1).

Cependant, le bundle wasm est toujours capable de monopoliser le fil d'exécution et d'allouer de la mémoire.


## Step 14 : Limiter l'exécution avec du fuel

Sur votre instance de [Config](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html), utiliser [Config::consume_fuel](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.consume_fuel) pour activer le support de la consommation d'une ressource (fuel) lors de l'exécution du bundle wasm.

Sur votre instance de [Store](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html), utiliser [Store::set_fuel](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html#method.set_fuel) pour initialiser le fuel à `100000`. Lors de l'exécution du bundle, cette valeur va descendre.

Utiliser également [Store::fuel_async_yield_interval](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html#method.fuel_async_yield_interval) avec une valeur de `5000` pour indiquer que l'exécution du bundle wasm doit rendre la main au runtime asynchrone sur cet interval de consommation. Dans un contexte concurrent avec plusieurs clients cela évitera de bloquer un thread Tokio sur l'exécution d'un seul bundle.

Tester la limite avec une boucle infinie.


## Step 15 : Limiter la consommation de mémoire

Dans la structure de données représentant l'état interne du runtime, ajouter un attributs représentant les limites, de type [StoreLimits](https://docs.rs/wasmtime/latest/wasmtime/struct.StoreLimits.html).
A la création de cet état interne, créer les limites en utilisant [StoreLimitsBuilder](https://docs.rs/wasmtime/latest/wasmtime/struct.StoreLimitsBuilder.html).
- Mettre la limite du nombre d'instances de module à `1`.
- Mettre la limite du nombre d'allocations de mémoire à `10`.
- Mettre la limite de la taille d'une allocation à `10_000_000`.

Enregistrer le limiter en utilisant [Store::limiter](https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html#method.limiter).

Tester la limite en avec des allocations :

```rust
let data = Box::new([0_u8 ; 15_000_000]);
```


## Step 16 : Bravo, c'est terminé !
