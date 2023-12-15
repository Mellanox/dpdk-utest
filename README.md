# dpdk-utest v3

## Assumptions

- Unit test is executed in Mellanox DPDK environment:
  - DUT and TG hosts are connected back-to-back.
  - Connected ports are of the same hardware type.
  - Each host in test setup has 2 ConnectX PF ports.
- Unit test will not validate port hardware type, Firmware and Mellanox OFED versions.

## Requirements

- Default SSH key for the `root` user must be configured on all tested hosts. 
  Use the `set-root-key.sh` Shell script to configure SSH `root` access.
  Current unit test implementation works with the default `id_rsa` key **only**.

## Test configuration file format

### Test setup section

```yaml

# Specify custom host configuration.
# If test did not provide `setup` or one of the `<host ID>` tags, 
# the test will run over existing interfaces. 

# Application definition        
<application id>:            # unique application identifier
    agent:                   # mandatory application type identifier: {testpmd|scapy}
    cmd:                     # optional
    path:                    # optional
    setup:                   # optional host setup
        domain: [ {nic|fdb}, {nic|fdb} ]
        vf: [  <VFs num port 0>,  <VFs num port 1>]
        sf: [ [<SF ids port 0>], [<SF ids port 1>] ]
```

### Test execution section

```yaml

phase:
    name: 
    <application id X1>: <commands>
    <application id X2>: <commands>
    result:
        <application id X1>: <expected output>

flow:
    -
        phases: [ <phase>, ... ]
        repeat: <phase repeat count>
```

- Application commands in phase executed in order of application tags appearance.

#### Network interfaces encoding

Unit test commands template cannot reference testpmd or scapy interfaces by name, because exact names differ between setups.
Test template use interface map. Map value is translated into host interface during test execution.

Unit test analyzes hosts network interfaces and Mellanox HCAs PCI addresses.
Results are stored in the hosts configuration file to speed up subsequent tests.

#### Testpmd PCI interfaces encoding

* PF PCI: `pciX X >= 0` 
  
  Example: `pci0: '0000:08:00.0'`

* VF PCI: `pciXvfY X >= 0, Y >= 0`

  Example: `pci0vf0: '0000:05:00.2'`

#### Network devices encoding

* PF: `pfX X>=0`

  Example: `pf0: enp8s0f0np0`

* SRIOV VF: `pfXvfY X>=0, Y >=0`

  Example: `pf0vf0: enp5s0f0v0`

* Network device representor: `pfXrfY X>=0, Y>=0` 

  Example: `pf0rf0: enp5s0f0npf0vf0`

## Hosts configuration file format

```yaml
<application id>:
    host: [<IP address> | DNS] # mandatory
    path:                      # optional application path
```


## Command line activation:

```
$ utest.py \
  --config <hosts configuration file> \
  --test <test configuration file> \
  [--fast] [--show]
```
