# dpdk-utest v3

## Assumptions

- Unit test is executed in Mellanox DPDK environment:
  - DUT and TG hosts are connected back-to-back.
  - Connected ports are of the same hardware type.
  - Each host in test setup has 2 ConnectX PF ports.
  - Unit test will not validate port hardware type, Firmware and Mellanox OFED versions.
  - 

- Host running unit test has SSH access to DUT and TG as the `root` user.

## Test configuration file format

### Test setup section

```yaml

# Specify custom host configuration.
# If test did not provide `setup` or one of the `<host ID>` tags, 
# the test will run over existing interfaces. 

# Application definition        
<application>:
    command_line:            # mandatory
    path: <application path> # optional
    setup: # optional setup tag
        domain: [ {nic|fdb}, {nic|fdb} ]
        vf: [ <VFs num port 0>, <VFs num port 1>]
        sf: [ <SFs num port 0>, <SFs num port 1>]

    # optional application tag with mandatory variable     
    <app tag>: @<app tag variable>    
```

#### Port names encoding

TBD

### Test execution section

```yaml

phase: @phase_id
    id: <phase id>
    <application_1>: <commands>
    <application_2>: <commands>
    result:
        <application_1>: <expected output>

utest:
    -
        phases: [ *phase[X0], ... *phase[Xn]]
        repeat: <phase repeat count>
```

- Application commands in phase executed in order of application tags appearence in the phase.


## Hosts configuration file format

```yaml
<application>: [<IP address> | DNS]
```

- Unit test analyzes hosts network interfaces and Mellanox HCAs PCI addresses.
  Results are stored in the hosts configuration file to speed up subsequent tests.

## Command line activation:

```shell
$ utest.py \
  --config <hosts configuration file> \
  --test <test configuration file>
```
