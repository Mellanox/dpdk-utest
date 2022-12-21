# dpdk-utest

## configuration file format:
```yaml
dut:  
  # DUT hostname or IP address  
  host:  
  # DUT username  
  username:  
  # DUT user password  
  password:  
  # DUT path to test program  
  path:  
  # DUT ports for EAL commandline  
  ports: [ PCI-port0, ... ]  
  
tg:  
  # TG hostname or IP address  
  host:  
  # TG username  
  username:  
  # TG password  
  password:  
  # general network interface 0  
  if0:
  # general network interface 1  
  if1:
  # PF
  pf[0,1]:
  # virtual interface - SRIOv or Auxiliary
  vf[0,1]:
  # representor
  rf[0,1]:
```

## test file format
``` yaml
prog: 'dpdk-testpmd -a PORT_0 -a PORT_1 -- -i'

flow:
  -
    phases: [ *phase1 ]
    repeat: 1
  -
    phases: [ *phase2, *phase3 ]
    repeat: 3
    
p: &phase
  # phase name - mandatory
  name:
  
  # PMD commands - optional
  pmd:
    - 
      # testpmd commands
      command:
      # string to match after command execution or None
      result:

  # TG command - optional     
  tg:

  # VM command - optional
  vm:

  # phase results to match - optional
  results:
    pmd:
    tg:
    vm:
      
  # phase repeat counter - optional
  repeat:
```      
  
### Phase execution order:
Execution order of `pmd` and `tg` keys depends on the keys position.
```
phase1:
    name:
    pmd:
    tg:

phase2:
    name:
    tg:
    pmd:
```

In `phase1` `tg` follows `pmd` while in `phase2` `pmd` follows `tg`.    
