snipe allows running C++ and python unit tests using just the test name.

So to run a C++ unit test, `test_foobar`, instead of having to

1. Find the file containing the test `test_foobar` in `CMakeLists.txt`
2. Build object containing the file.
3. Run the test using the path to the object

one can run `snipe --cc test_foobar` and the above sequence is automated.

The build type is read from `.env` so the test variant is run for `DEBUG` or `RELEASE` depending on what `.env` is set
to.

Similarly to run a python test, `test_foobar`, instead of having to

1. Find the class name containing the test
2. Find the path to the file containing the test
3. Construct the ducktape command using the path and class name, and run that

one can run `snipe --py test_foobar` and the above sequence is automated.

### Building and running

The project can be built with cargo. The binary should then be copied somewhere to the path, and the commands below
should be run from within the redpanda project root.

### Running a C++ unit test

```shell
$ snipe --cc test_aws_credentials
```

### Running a ducktape test

```shell
$ snipe --py test_basic_assignment
```

### Edit command before running

Use the `-e` flag. This presents a prompt before running each command, allowing addition of custom flags etc.

```shell
$ snipe -e --cc test_aws_credentials
```

### Customizing behavior

On first run, snipe creates these config files. On subsequent runs, these are read first. Change these files to
customize behavior:

#### Command templates

Path: `~/.config/snipe/command_config.json`.

Allows customizing the commands executed. Default values are:

```json
{
  "command_mappings": {
    "compile": "ninja -C vbuild/{{build_type}}/clang -j 25 bin/{{test_obj}}",
    "run": "./tools/cmake_test.py --binary {{pwd}}/vbuild/{{build_type}}/clang/bin/{{test_obj}} {{test_tag_arg}} -- -c1",
    "duck": "task rp:run-ducktape-tests DUCKTAPE_ARGS=\"{{test_path}} {{test_args}}\""
  }
}
```

The placeholders are filled in at runtime. `compile` and `run` are used for C++ tests. `duck` is used to run the
ducktape tests.

#### Environment variables

Path: `~/.config/snipe/command_env.json`.

Allows setting environment variables for all commands executed. Default values
are:

```json
{
  "envs": {
    "RP_TRIM_LOGS": "false",
    "ENABLE_GIT_HASH": "OFF",
    "REDPANDA_LOG_LEVEL": "trace",
    "ENABLE_GIT_VERSION": "OFF"
  }
}
```

These values are injected into all tests (C++ and python).

#### Test scan paths

Path: `~/.config/snipe/scan_config.json`. Allows setting paths to scan tests in. Default values are:

```json
{
  "cc_test_root": "src/v",
  "py_test_root": "tests/rptest"
}
```

### How test runs are automated

Both C++ and python tests are parsed and stored in JSON files. The files can be found in:

1. `~.local/share/snipe/cc.json`
2. `~.local/share/snipe/py.json`

The parsing is done using a custom parser for `CMakeLists.txt` and C++ test files
and [rust-python](https://github.com/RustPython/RustPython) for parsing the python code.

Once the JSON files are populated, the test name is searched in them to construct the commands to run. If the test is
not found, it is assumed that it may have been recently added,
and the scan is done once again. Once populated, the source paths are not re-scanned on future runs unless a test is
found missing.

If multiple targets are found matching a test name (a common scenario for generic test names), a list is presented and a
selection must be made.

# Security considerations

Because the commands are run via a wrapper (teetty) to preserve color information, user input is echoed to screen.

This can be a problem in some cases. For example, if sudoers is set up to require a password to be entered for some
command, the password will be echoed to screen in plaintext instead of the usual Linux hidden text.

#### TODOs

- [ ] Allow disabling color to suppress text echoed back
- [ ] Allow clearing cached data to force a rescan
- [ ] Pass through extra arguments to tests
- [ ] Support python tests not annotated with `@cluster`
- [ ] Support googletest
