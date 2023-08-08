# quota-limit-monitor
Used to get the limits, current usages of some of the service quotas.

## Supported Services and Quota
### EC2
- No of vCPUs in On-Demand Standard (A, C, D, H, I, M, R, T, Z) instances
- Elastic IP Address
## Setting up and running the project

- Clone the repo.
- Set up the aws profile in your system.

One of the following actions can be run at a time:
### 1. `--limit-status`
Shows the current usages/limit of all the supported services and quotas.

Command Example -
```shell
cargo run -- --limit-status
```

Output Format -
```
<quota-name>: <current_usage>/<quota_limit>,
<quota-name>: <current_usage>/<quota_limit>,
...
```

### 2. `--limit-increase`
Requests an increase in the limit of any of the supported quotas. This also stores all the succesfully sent requests to `requests.log`.

Requires two values - 

1. `quota-name`: Value must be `ec2` or `elastic_ip`.
2. `quota-value`: Value must be a number/float greater than the current limit of the quota.

Command Example -
```shell
cargo run -- --limit-increase --quota-name ec2 --quota-value 36
```

Output Format -
```
Service quota increase requested
Request ID: <request-id>
```

### 3. `--request-status`
Fetches the status of the requested quota increase.

Requires one value - 

1. `request-id`: Value must be a valid request id. You can get it from `requests.log`.

Command Example -
```shell
cargo run -- --request-status --request-id  <request-id>
```

Output Format -
```
Status: status
```