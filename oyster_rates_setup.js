// Script to setup rates file for oyster.

// Pre : npm install @aws-sdk/client-pricing && npm install @aws-sdk/client-ec2
// Pre : make it executable : chmod +x oyster_rates_setup.js

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
            return instanceType.InstanceType
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
                return instanceType.InstanceType
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
async function getEc2Prices(instanceType, premium) {
    const params = {
        ServiceCode: 'AmazonEC2',
        Filters: [
            {
                Type: 'TERM_MATCH',
                Field: 'instanceType',
                Value: instanceType
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
        }).map((instance) => ({
            region: instance.product.attributes.regionCode,
            instance: instance.product.attributes.instanceType,
            min_rate: Math.ceil(parseFloat(instance.terms.OnDemand[Object.keys(instance.terms.OnDemand)[0]]
                .priceDimensions[Object.keys(instance.terms.OnDemand[Object.keys(instance.terms.OnDemand)[0]]
                    .priceDimensions)[0]].pricePerUnit.USD).toFixed(6) * 1e6 * (100 + premium) / 360000)
        }))


        // console.log(util.inspect(productsFiltered, false, null, true))

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

    const ec2InstanceTypes = await getAllInstanceTypesWithNitro();

    const excludedRegions = [];
    const excludedInstances = [];
    // Listed General purpose and compute optimized instances except the ones soring volumes
    const selectInstanceFamiliesOnly = true;

    const selectRegionsOnly = false;
    const selectRegions = [];
    const selectInstanceFamilies = ["m5.", "m5a.", "m5n.", "m5zn.", "m6a.", "m6g.", "m6i.", "m6in.", "m7g.", "c5.", "c5a.", "c5n.", "c6a.", "c6g.", "c6gn.", "c6i.", "c6in.", "c7g.", "c7gn.", "hpc6a.", "hpc7g.",];
    let products = [];
    for (let i = 0; i < ec2InstanceTypes.length; i++) {
        const res = await getEc2Prices(ec2InstanceTypes[i], premium);
        products.push(...res);
    }

    // Change into [{region,[{instance,min_rate}]}] format
    const result = products.reduce((newProds, curr) => {
        if (excludedInstances.includes(curr.instance) || excludedRegions.includes(curr.region)) return newProds;
        else if (selectInstanceFamiliesOnly && !selectInstanceFamilies.some(prefix => curr.instance.startsWith(prefix))) return newProds;
        else if (selectRegionsOnly && !selectRegions.includes(curr.region)) return newProds;

        const found = newProds.find(el => el.region === curr.region);

        if (found) {
            found.rate_cards.push({ instance: curr.instance, min_rate: curr.min_rate });
        } else {
            newProds.push({
                region: curr.region,
                rate_cards: [{ instance: curr.instance, min_rate: curr.min_rate },]
            });
        }
        return newProds;
    }, []);
    // console.log(util.inspect(result, false, null, true))

    // Write to .marlin folder
    const data = JSON.stringify(result, null, 2);
    fs.writeFile(location, data, (error) => {
        if (error) {
            console.error(error);

            throw error;
        }

        console.log('rates file written correctly');
    });
}

run();
