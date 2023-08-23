// Script to setup rates file for oyster.

// To use for or exclude certain regions and instance types modify the run function as needed

const fs = require("fs");
const { PricingClient, GetProductsCommand } = require("@aws-sdk/client-pricing");
const { EC2Client, DescribeInstanceTypesCommand } = require('@aws-sdk/client-ec2');

const ec2Client = new EC2Client();
const pricingClient = new PricingClient();

// Function to get all the instance types using the nitro hypervisor i.e. supporting secure enclaves with vcpus > 1
async function getAllInstanceTypesWithNitro() {
    const input = {};
    const command = new DescribeInstanceTypesCommand(input);

    try {
        const response = await ec2Client.send(command);

        let instanceTypes = response.InstanceTypes.filter((instanceType) => {
            return (instanceType.Hypervisor === 'nitro') &&
                ((instanceType.ProcessorInfo.SupportedArchitectures[0] === 'x86_64' && instanceType.VCpuInfo.DefaultVCpus >= 4)
                    || (instanceType.ProcessorInfo.SupportedArchitectures[0] === 'arm64' && instanceType.VCpuInfo.DefaultVCpus >= 2));
        }).map((instanceType) => {
            return [instanceType.InstanceType, { arch: instanceType.ProcessorInfo.SupportedArchitectures[0].replace("x86_64", "amd64") }];
        });
        let remaining = response.NextToken;
        while (remaining != null) {
            const input = {
                NextToken: remaining
            };
            const command = new DescribeInstanceTypesCommand(input);
            const response = await ec2Client.send(command);
            const data = response.InstanceTypes.filter((instanceType) => {
                return (instanceType.Hypervisor === 'nitro') &&
                    ((instanceType.ProcessorInfo.SupportedArchitectures[0] === 'x86_64' && instanceType.VCpuInfo.DefaultVCpus >= 4)
                        || (instanceType.ProcessorInfo.SupportedArchitectures[0] === 'arm64' && instanceType.VCpuInfo.DefaultVCpus >= 2));
            }).map((instanceType) => {
                return [instanceType.InstanceType, { arch: instanceType.ProcessorInfo.SupportedArchitectures[0].replace("x86_64", "amd64") }];
            });
            instanceTypes.push(...data);
            remaining = response.NextToken;
        }
        return instanceTypes;
    } catch (error) {
        console.error(error);
        return [];
    }
}

// Function to get the price of DedicatedUsage of an instance type in all supported regions 
async function getEc2Prices(instanceTypeWithMetadata, premium) {
    const params = {
        ServiceCode: 'AmazonEC2',
        Filters: [
            {
                Type: 'TERM_MATCH',
                Field: 'instanceType',
                Value: instanceTypeWithMetadata[0]
            },
            {
                Type: 'TERM_MATCH',
                Field: 'operatingSystem',
                Value: 'Linux'
            },
            {
                Type: 'TERM_MATCH',
                Field: 'preInstalledSw',
                Value: 'NA'
            },
        ]
    };

    const command = new GetProductsCommand(params);

    try {
        const response = await pricingClient.send(command);

        const products = response.PriceList.map((product) => JSON.parse(product));

        const productsFiltered = products.filter(i => {
            return i.product.attributes.usagetype.includes('DedicatedUsage')
        }).map((instance) => {
            const on_demand_key = Object.keys(instance.terms.OnDemand)[0];
            const price_dimension_key = Object.keys(instance.terms.OnDemand[on_demand_key].priceDimensions)[0];
            return {
                region: instance.product.attributes.regionCode,
                instance: instance.product.attributes.instanceType,
                min_rate: BigInt(parseFloat(instance.terms.OnDemand[on_demand_key]
                    .priceDimensions[price_dimension_key].pricePerUnit.USD * 1e6 || 0).toFixed(0)) * BigInt(100 + premium) * BigInt(1e12) / BigInt(360000),
                cpus: instance.product.attributes.vcpu,
                memory: instance.product.attributes.memory,
                arch: instanceTypeWithMetadata[1].arch
            };
        })

        const list = productsFiltered.filter(i => { return i.min_rate > 0 })

        return list;
    } catch (error) {
        console.error(error);
    }
}

async function run() {
    const args = process.argv;
    const location = args[2];
    let premium = args[3];

    if (location == null) {
        console.log("Please pass the file location");
        process.exit(1);
    }

    if (premium == null) {
        console.log("Please pass the premium");
        process.exit(1);
    } else if (isNaN(premium)) {
        console.log("Please pass a valid number for premium");
        process.exit(1);
    }

    premium = parseInt(premium);

    const excludedRegions = [];
    const excludedInstances = [];
    // Listed General purpose and compute optimized instances except the ones soring volumes
    const selectInstanceFamiliesOnly = true;
    const selectInstanceFamilies = ["m5.", "m5a.", "m5n.", "m5zn.", "m6a.", "m6g.", "m6i.", "m6in.", "m7g.", "c5.", "c5a.", "c5n.", "c6a.", "c6g.", "c6gn.", "c6i.", "c6in.", "c7g.", "c7gn.", "hpc6a.", "hpc7g.",];

    const ec2InstanceTypesWithMetadata = (await getAllInstanceTypesWithNitro()).filter(type => !selectInstanceFamiliesOnly || selectInstanceFamilies.some(prefix => type[0].startsWith(prefix)));

    const selectRegionsOnly = true;
    const selectRegions = [
        "us-east-1",
        "us-east-2",
        "us-west-1",
        "us-west-2",
        "ca-central-1",
        "sa-east-1",
        "eu-north-1",
        "eu-west-3",
        "eu-west-2",
        "eu-west-1",
        "eu-central-1",
        "eu-central-2",
        "eu-south-1",
        "eu-south-2",
        "me-south-1",
        "me-central-1",
        "af-south-1",
        "ap-south-1",
        "ap-south-2",
        "ap-northeast-1",
        "ap-northeast-2",
        "ap-northeast-3",
        "ap-southeast-1",
        "ap-southeast-2",
        "ap-southeast-3",
        "ap-southeast-4",
        "ap-east-1",
    ];
    let products = [];
    for (let i = 0; i < ec2InstanceTypesWithMetadata.length; i++) {
        const res = await getEc2Prices(ec2InstanceTypesWithMetadata[i], premium);
        products.push(...res);
    }

    // Change into [{region,[{instance,min_rate,cpu,memory,arch}]}] format
    const result = products.reduce((newProds, curr) => {
        if (excludedInstances.includes(curr.instance) || excludedRegions.includes(curr.region)) return newProds;
        else if (selectRegionsOnly && !selectRegions.includes(curr.region)) return newProds;

        const found = newProds.find(el => el.region === curr.region);

        if (found) {
            found.rate_cards.push({ instance: curr.instance, min_rate: curr.min_rate, cpu: curr.cpus, memory: curr.memory, arch: curr.arch });
        } else {
            newProds.push({
                region: curr.region,
                rate_cards: [{ instance: curr.instance, min_rate: curr.min_rate, cpu: curr.cpus, memory: curr.memory, arch: curr.arch },]
            });
        }
        return newProds;
    }, []);
    // console.log(util.inspect(result, false, null, true))

    // Write to .marlin folder
    const data = JSON.stringify(result, (_, v) => typeof v === 'bigint' ? v.toString() : v, 2);
    fs.writeFile(location, data, (error) => {
        if (error) {
            console.error(error);

            throw error;
        }

        console.log('rates file written correctly');
    });
}

run();
